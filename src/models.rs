use anyhow::{bail, Result};
use serde::Serialize;
use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Queued,
    Implementing,
    Verifying,
    Done,
    NeedsHuman,
}

impl Status {
    pub fn as_str(self) -> &'static str {
        match self {
            Status::Queued => "queued",
            Status::Implementing => "implementing",
            Status::Verifying => "verifying",
            Status::Done => "done",
            Status::NeedsHuman => "needs_human",
        }
    }
}

impl FromStr for Status {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> Result<Self> {
        Ok(match s {
            "queued" => Status::Queued,
            "implementing" => Status::Implementing,
            "verifying" => Status::Verifying,
            "done" => Status::Done,
            "needs_human" => Status::NeedsHuman,
            other => bail!("invalid status: {other}"),
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkType {
    CodeImplementation,
    Investigation,
    Documentation,
    Design,
}

impl WorkType {
    pub fn as_str(self) -> &'static str {
        match self {
            WorkType::CodeImplementation => "code_implementation",
            WorkType::Investigation => "investigation",
            WorkType::Documentation => "documentation",
            WorkType::Design => "design",
        }
    }
}

impl FromStr for WorkType {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> Result<Self> {
        Ok(match s {
            "code_implementation" => WorkType::CodeImplementation,
            "investigation" => WorkType::Investigation,
            "documentation" => WorkType::Documentation,
            "design" => WorkType::Design,
            other => bail!("invalid work type: {other}"),
        })
    }
}

#[derive(Debug, Serialize)]
pub struct Ticket {
    pub id: i64,
    pub spec_id: i64,
    pub title: String,
    pub description: String,
    pub definitions_of_done: serde_json::Value,
    pub status: String,
    pub attempts: i64,
    pub human_context: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct Task {
    pub id: i64,
    pub ticket_id: i64,
    pub title: String,
    pub work_type: String,
    pub objective: String,
    pub acceptance_criteria: serde_json::Value,
    pub context: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_roundtrip() {
        for s in ["queued", "implementing", "verifying", "done", "needs_human"] {
            let parsed = Status::from_str(s).unwrap();
            assert_eq!(parsed.as_str(), s);
        }
        assert!(Status::from_str("bogus").is_err());
    }

    #[test]
    fn work_type_roundtrip() {
        for s in [
            "code_implementation",
            "investigation",
            "documentation",
            "design",
        ] {
            assert_eq!(WorkType::from_str(s).unwrap().as_str(), s);
        }
        assert!(WorkType::from_str("refactor").is_err());
    }
}
