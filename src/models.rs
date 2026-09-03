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

#[derive(Debug, Serialize)]
pub struct Ticket {
    pub id: i64,
    pub spec_id: i64,
    pub title: String,
    pub description: String,
    pub acceptance_criteria: serde_json::Value,
    pub status: String,
    pub attempts: i64,
    pub human_context: Option<String>,
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
}
