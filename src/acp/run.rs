use super::config::AgentSpec;
use super::event::{Event, EventSink};
use agent_client_protocol::{self as acp, Agent as _};
use anyhow::Result;
use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;
use tokio_util::compat::{TokioAsyncReadCompatExt, TokioAsyncWriteCompatExt};

pub fn run(agent: &AgentSpec, prompt: &str, cwd: &Path, sink: &mut dyn EventSink) -> Result<()> {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    rt.block_on(async {
        tokio::task::LocalSet::new()
            .run_until(run_async(agent, prompt, cwd, sink))
            .await
    })
}

fn is_agent_process_exit(msg: &str) -> bool {
    let lower = msg.to_ascii_lowercase();
    lower.contains("broken pipe")
        || lower.contains("closed")
        || lower.contains("connection reset")
        || lower.contains("eof")
        || lower.contains("shut down")
        || lower.contains("server shut down unexpectedly")
}

fn map_acp_err(err: acp::Error) -> anyhow::Error {
    if is_agent_process_exit(&err.to_string()) {
        anyhow::anyhow!("agent process exited: {err}")
    } else {
        anyhow::anyhow!("{err}")
    }
}

fn stop_reason_str(reason: acp::StopReason) -> String {
    match reason {
        acp::StopReason::EndTurn => "end_turn".into(),
        acp::StopReason::MaxTokens => "max_tokens".into(),
        acp::StopReason::MaxTurnRequests => "max_turn_requests".into(),
        acp::StopReason::Refusal => "refusal".into(),
        acp::StopReason::Cancelled => "cancelled".into(),
        other => serde_json::to_value(other)
            .ok()
            .and_then(|v| v.as_str().map(str::to_string))
            .unwrap_or_else(|| "unknown".into()),
    }
}

async fn run_async(
    agent: &AgentSpec,
    prompt: &str,
    cwd: &Path,
    sink: &mut dyn EventSink,
) -> Result<()> {
    let mut child = tokio::process::Command::new(&agent.command)
        .args(&agent.args)
        .envs(&agent.env)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::inherit())
        .kill_on_drop(true)
        .spawn()
        .map_err(|e| anyhow::anyhow!("failed to spawn {}: {e}", agent.command))?;
    let stdin = child.stdin.take().unwrap().compat_write();
    let stdout = child.stdout.take().unwrap().compat();

    let pending: Rc<RefCell<Vec<Event>>> = Rc::new(RefCell::new(Vec::new()));
    let (conn, handle_io) = acp::ClientSideConnection::new(
        BoardClient {
            pending: pending.clone(),
        },
        stdin,
        stdout,
        |fut| {
            tokio::task::spawn_local(fut);
        },
    );
    tokio::task::spawn_local(handle_io);

    let init = conn
        .initialize(acp::InitializeRequest::new(acp::ProtocolVersion::V1))
        .await
        .map_err(map_acp_err)?;
    sink.emit(Event::Initialized {
        protocol_version: serde_json::to_value(&init.protocol_version)?,
        agent_name: init.agent_info.as_ref().map(|i| i.name.clone()),
    })?;

    let abs_cwd = if cwd.is_absolute() {
        cwd.to_path_buf()
    } else {
        cwd.canonicalize()?
    };
    let session = conn
        .new_session(acp::NewSessionRequest::new(abs_cwd))
        .await
        .map_err(map_acp_err)?;
    let session_id = session.session_id.to_string();
    sink.emit(Event::Session {
        session_id: session_id.clone(),
    })?;

    let prompt_result = conn
        .prompt(acp::PromptRequest::new(
            session.session_id.clone(),
            vec![prompt.into()],
        ))
        .await
        .map_err(map_acp_err)?;

    for event in pending.borrow_mut().drain(..) {
        sink.emit(event)?;
    }

    sink.emit(Event::Result {
        session_id,
        stop_reason: stop_reason_str(prompt_result.stop_reason),
    })?;

    let _ = child.start_kill();
    let _ = child.wait().await;
    Ok(())
}

struct BoardClient {
    pending: Rc<RefCell<Vec<Event>>>,
}

#[async_trait::async_trait(?Send)]
impl acp::Client for BoardClient {
    async fn request_permission(
        &self,
        args: acp::RequestPermissionRequest,
    ) -> Result<acp::RequestPermissionResponse, acp::Error> {
        self.pending.borrow_mut().push(Event::DeniedPermission {
            session_id: args.session_id.to_string(),
            tool: Some(args.tool_call.tool_call_id.to_string()),
        });
        let outcome = args
            .options
            .iter()
            .find(|o| {
                matches!(
                    o.kind,
                    acp::PermissionOptionKind::RejectOnce | acp::PermissionOptionKind::RejectAlways
                )
            })
            .map(|o| {
                acp::RequestPermissionOutcome::Selected(acp::SelectedPermissionOutcome::new(
                    o.option_id.clone(),
                ))
            })
            .unwrap_or(acp::RequestPermissionOutcome::Cancelled);
        Ok(acp::RequestPermissionResponse::new(outcome))
    }

    async fn session_notification(&self, args: acp::SessionNotification) -> Result<(), acp::Error> {
        let update =
            serde_json::to_value(&args.update).map_err(|_| acp::Error::internal_error())?;
        self.pending.borrow_mut().push(Event::Update {
            session_id: args.session_id.to_string(),
            update,
        });
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn map_acp_err_server_shut_down_is_agent_process_exited() {
        let err = acp::Error::internal_error().data("server shut down unexpectedly");
        let mapped = map_acp_err(err.clone());
        let msg = mapped.to_string();
        assert!(msg.starts_with("agent process exited: "), "{msg}");
        assert!(msg.contains(&err.to_string()), "{msg}");
    }

    #[test]
    fn map_acp_err_connection_closed_is_agent_process_exited() {
        let err =
            acp::Error::internal_error().data("connection closed before request could be sent");
        let msg = map_acp_err(err).to_string();
        assert!(msg.contains("agent process exited"), "{msg}");
    }

    #[test]
    fn map_acp_err_other_internal_keeps_original() {
        let err = acp::Error::internal_error().data("failed to deserialize response");
        let mapped = map_acp_err(err.clone());
        let msg = mapped.to_string();
        assert!(!msg.contains("agent process exited"), "{msg}");
        assert_eq!(msg, err.to_string());
    }
}
