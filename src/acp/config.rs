use anyhow::{bail, Context, Result};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AgentSpec {
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub env: BTreeMap<String, String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct AgentsFile {
    #[serde(default)]
    agents: BTreeMap<String, AgentSpec>,
}

fn agent_path(root: &Path) -> PathBuf {
    root.join(".abd").join("agents.yaml")
}

fn load_file(path: &Path) -> Result<BTreeMap<String, AgentSpec>> {
    let text = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read {}", path.display()))?;
    let parsed: AgentsFile = serde_yaml::from_str(&text)
        .map_err(|e| anyhow::anyhow!("invalid agents file {}: {e}", path.display()))?;
    let mut out = parsed.agents;
    for (id, spec) in &out {
        if spec.command.trim().is_empty() {
            bail!(
                "invalid agents file {}: agent {id} has empty command",
                path.display()
            );
        }
    }
    for spec in out.values_mut() {
        spec.command = spec.command.trim().to_string();
    }
    Ok(out)
}

fn merge_file(into: &mut BTreeMap<String, AgentSpec>, path: &Path, required: bool) -> Result<()> {
    if !path.is_file() {
        if required {
            bail!("agents config file not found: {}", path.display());
        }
        return Ok(());
    }
    let layer = load_file(path)?;
    into.extend(layer);
    Ok(())
}

pub fn load_agents(
    home: Option<&Path>,
    workspace_dir: &Path,
    extra: Option<&Path>,
) -> Result<BTreeMap<String, AgentSpec>> {
    let mut agents = BTreeMap::new();
    if let Some(home) = home {
        merge_file(&mut agents, &agent_path(home), false)?;
    }
    merge_file(&mut agents, &agent_path(workspace_dir), false)?;
    if let Some(extra) = extra {
        merge_file(&mut agents, extra, true)?;
    }
    Ok(agents)
}

pub fn get_agent<'a>(agents: &'a BTreeMap<String, AgentSpec>, id: &str) -> Result<&'a AgentSpec> {
    agents
        .get(id)
        .ok_or_else(|| anyhow::anyhow!("unknown agent {id}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_yaml(dir: &std::path::Path, rel: &str, body: &str) {
        let path = dir.join(rel);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(path, body).unwrap();
    }

    #[test]
    fn workspace_replaces_user_id_and_keeps_user_only() {
        let tmp = tempfile::TempDir::new().unwrap();
        let home = tmp.path().join("home");
        let ws = tmp.path().join("ws");
        write_yaml(
            &home,
            ".abd/agents.yaml",
            "agents:\n  user_only:\n    command: u\n  shared:\n    command: from-user\n",
        );
        write_yaml(
            &ws,
            ".abd/agents.yaml",
            "agents:\n  shared:\n    command: from-ws\n",
        );
        let agents = load_agents(Some(&home), &ws, None).unwrap();
        assert_eq!(agents["user_only"].command, "u");
        assert_eq!(agents["shared"].command, "from-ws");
    }

    #[test]
    fn extra_replaces_workspace_id() {
        let tmp = tempfile::TempDir::new().unwrap();
        let ws = tmp.path().join("ws");
        write_yaml(&ws, ".abd/agents.yaml", "agents:\n  a:\n    command: ws\n");
        let extra = tmp.path().join("extra.yaml");
        std::fs::write(&extra, "agents:\n  a:\n    command: extra\n").unwrap();
        let agents = load_agents(None, &ws, Some(&extra)).unwrap();
        assert_eq!(agents["a"].command, "extra");
    }

    #[test]
    fn missing_files_yield_empty_map() {
        let tmp = tempfile::TempDir::new().unwrap();
        let agents = load_agents(Some(tmp.path()), tmp.path(), None).unwrap();
        assert!(agents.is_empty());
    }

    #[test]
    fn extra_missing_file_errors_with_path() {
        let tmp = tempfile::TempDir::new().unwrap();
        let missing = tmp.path().join("nope.yaml");
        let err = load_agents(None, tmp.path(), Some(&missing)).unwrap_err();
        assert!(err.to_string().contains(&missing.display().to_string()));
    }

    #[test]
    fn unknown_key_errors_with_path() {
        let tmp = tempfile::TempDir::new().unwrap();
        let extra = tmp.path().join("bad.yaml");
        std::fs::write(&extra, "agents:\n  a:\n    command: x\n    nope: 1\n").unwrap();
        let err = load_agents(None, tmp.path(), Some(&extra)).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains(&extra.display().to_string()), "{msg}");
        assert!(
            msg.contains("nope") && msg.contains("unknown field"),
            "{msg}"
        );
    }

    #[test]
    fn duplicate_agent_id_in_one_file_keeps_last() {
        let tmp = tempfile::TempDir::new().unwrap();
        let extra = tmp.path().join("dup.yaml");
        std::fs::write(
            &extra,
            "agents:\n  a:\n    command: one\n  a:\n    command: two\n",
        )
        .unwrap();
        let agents = load_agents(None, tmp.path(), Some(&extra)).unwrap();
        assert_eq!(agents["a"].command, "two");
    }

    #[test]
    fn empty_command_errors_with_path() {
        let tmp = tempfile::TempDir::new().unwrap();
        let extra = tmp.path().join("empty.yaml");
        std::fs::write(&extra, "agents:\n  a:\n    command: \"  \"\n").unwrap();
        let err = load_agents(None, tmp.path(), Some(&extra)).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains(&extra.display().to_string()));
        assert!(msg.contains("command"));
    }

    #[test]
    fn get_agent_unknown_id_mentions_id() {
        let map = std::collections::BTreeMap::new();
        let err = get_agent(&map, "nope").unwrap_err();
        assert!(err.to_string().contains("nope"));
    }
}
