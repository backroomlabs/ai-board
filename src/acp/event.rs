use anyhow::Result;
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Event {
    Initialized {
        protocol_version: serde_json::Value,
        #[serde(skip_serializing_if = "Option::is_none")]
        agent_name: Option<String>,
    },
    Session {
        session_id: String,
    },
    Update {
        session_id: String,
        update: serde_json::Value,
    },
    DeniedPermission {
        session_id: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        tool: Option<String>,
    },
    Result {
        session_id: String,
        stop_reason: String,
    },
}

pub trait EventSink {
    fn emit(&mut self, event: Event) -> Result<()>;
}

pub struct JsonlStdoutSink;

impl EventSink for JsonlStdoutSink {
    fn emit(&mut self, event: Event) -> Result<()> {
        println!("{}", to_json_line(&event)?);
        Ok(())
    }
}

pub struct VecSink(pub Vec<Event>);

impl EventSink for VecSink {
    fn emit(&mut self, event: Event) -> Result<()> {
        self.0.push(event);
        Ok(())
    }
}

pub fn to_json_line(event: &Event) -> Result<String> {
    Ok(serde_json::to_string(event)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn initialized_omits_agent_name_when_none() {
        let line = to_json_line(&Event::Initialized {
            protocol_version: json!(1),
            agent_name: None,
        })
        .unwrap();
        let v: serde_json::Value = serde_json::from_str(&line).unwrap();
        assert_eq!(v["type"], "initialized");
        assert!(v.get("agent_name").is_none());
        assert_eq!(v["protocol_version"], 1);
    }

    #[test]
    fn initialized_includes_agent_name() {
        let line = to_json_line(&Event::Initialized {
            protocol_version: json!(1),
            agent_name: Some("mock".into()),
        })
        .unwrap();
        let v: serde_json::Value = serde_json::from_str(&line).unwrap();
        assert_eq!(v["agent_name"], "mock");
    }

    #[test]
    fn update_embeds_raw_object() {
        let line = to_json_line(&Event::Update {
            session_id: "s1".into(),
            update: json!({"sessionUpdate": "agent_message_chunk"}),
        })
        .unwrap();
        let v: serde_json::Value = serde_json::from_str(&line).unwrap();
        assert_eq!(v["type"], "update");
        assert_eq!(v["session_id"], "s1");
        assert_eq!(v["update"]["sessionUpdate"], "agent_message_chunk");
    }

    #[test]
    fn denied_permission_omits_tool_when_none() {
        let line = to_json_line(&Event::DeniedPermission {
            session_id: "s1".into(),
            tool: None,
        })
        .unwrap();
        let v: serde_json::Value = serde_json::from_str(&line).unwrap();
        assert_eq!(v["type"], "denied_permission");
        assert!(v.get("tool").is_none());
    }

    #[test]
    fn result_end_turn() {
        let line = to_json_line(&Event::Result {
            session_id: "s1".into(),
            stop_reason: "end_turn".into(),
        })
        .unwrap();
        let v: serde_json::Value = serde_json::from_str(&line).unwrap();
        assert_eq!(v["type"], "result");
        assert_eq!(v["stop_reason"], "end_turn");
    }
}
