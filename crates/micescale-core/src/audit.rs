use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::CoreError;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AuditEvent {
    pub ts: u64,
    pub event: String,
    pub carrier: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub control_server: Option<String>,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub node_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

pub fn append(path: &Path, event: &AuditEvent) -> Result<(), CoreError> {
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|error| {
            CoreError::Audit(format!(
                "cannot open audit log at {}: {error}",
                path.display()
            ))
        })?;
    let line = serde_json::to_string(event).expect("audit event serializes");
    writeln!(file, "{line}").map_err(|error| {
        CoreError::Audit(format!(
            "cannot write audit event to {}: {error}",
            path.display()
        ))
    })
}

pub fn tail(path: &Path, limit: usize) -> Result<Vec<AuditEvent>, CoreError> {
    let source = match fs::read_to_string(path) {
        Ok(source) => source,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(CoreError::Audit(error.to_string())),
    };
    let mut events = Vec::new();
    for (index, line) in source.lines().enumerate() {
        match serde_json::from_str::<AuditEvent>(line) {
            Ok(event) => events.push(event),
            Err(error) => {
                return Err(CoreError::Audit(format!(
                    "corrupt audit line {}: {error}",
                    index + 1
                )));
            }
        }
    }
    let skip = events.len().saturating_sub(limit.max(1));
    Ok(events.into_iter().skip(skip).collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn sample(event: &str) -> AuditEvent {
        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        AuditEvent {
            ts,
            event: event.into(),
            carrier: "headscale".into(),
            control_server: Some("https://headscale.example.com".into()),
            status: "ok".into(),
            node_name: Some("smoke-node".into()),
            detail: None,
        }
    }

    #[test]
    fn append_and_tail_round_trip() {
        let dir = std::env::temp_dir().join("micescale-audit-test");
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("audit.jsonl");
        let _ = fs::remove_file(&path);
        append(&path, &sample("enroll")).unwrap();
        append(&path, &sample("leave")).unwrap();
        let events = tail(&path, 10).unwrap();
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].event, "enroll");
        assert_eq!(events[1].event, "leave");
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn tail_respects_limit() {
        let dir = std::env::temp_dir().join("micescale-audit-test-limit");
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("audit.jsonl");
        let _ = fs::remove_file(&path);
        for event in ["a", "b", "c", "d"] {
            append(&path, &sample(event)).unwrap();
        }
        let events = tail(&path, 2).unwrap();
        assert_eq!(
            events.iter().map(|e| e.event.as_str()).collect::<Vec<_>>(),
            vec!["c", "d"]
        );
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn tail_of_missing_log_is_empty() {
        let events = tail(Path::new("/nonexistent/micescale/audit.jsonl"), 10).unwrap();
        assert!(events.is_empty());
    }
}
