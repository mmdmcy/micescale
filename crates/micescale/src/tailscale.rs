use std::collections::HashMap;
use std::env;
use std::path::PathBuf;
use std::process::Command;

use serde::Deserialize;

use micescale_core::paths::TAILSCALE_BIN_ENV;

use crate::AppError;

pub fn binary() -> Result<String, AppError> {
    if let Some(custom) = env::var_os(TAILSCALE_BIN_ENV).filter(|value| !value.is_empty()) {
        let path = PathBuf::from(custom);
        if !path.exists() {
            return Err(AppError::Operational(format!(
                "{} points to {} which does not exist",
                TAILSCALE_BIN_ENV,
                path.display()
            )));
        }
        return Ok(path.to_string_lossy().into_owned());
    }
    Ok("tailscale".into())
}

pub fn run(args: &[&str]) -> Result<Output, AppError> {
    let binary = binary()?;
    let output = Command::new(&binary).args(args).output().map_err(|error| {
        AppError::Operational(format!(
            "cannot execute {binary}: {error}; is the tailscale client installed?"
        ))
    })?;
    Ok(Output {
        status: output.status.code(),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    })
}

pub struct Output {
    pub status: Option<i32>,
    pub stdout: String,
    pub stderr: String,
}

impl Output {
    pub fn ok(&self, context: &str) -> Result<(), AppError> {
        if self.status == Some(0) {
            Ok(())
        } else {
            let detail = if self.stderr.trim().is_empty() {
                self.stdout.trim()
            } else {
                self.stderr.trim()
            };
            Err(AppError::Operational(format!("{context} failed: {detail}")))
        }
    }
}

pub fn up(server: &str, node_name: Option<&str>, authkey: &str) -> Result<(), AppError> {
    let mut args = vec![
        "up",
        "--login-server",
        server,
        "--authkey",
        authkey,
        // Required when switching a device from another control server (for
        // example Tailscale cloud) to a self-hosted Headscale.
        "--force-reauth",
    ];
    if let Some(name) = node_name {
        args.push("--hostname");
        args.push(name);
    }
    run(&args)?.ok("tailscale up")
}

pub fn logout() -> Result<(), AppError> {
    run(&["logout"])?.ok("tailscale logout")
}

pub fn status_json() -> Result<TailscaleStatus, AppError> {
    let output = run(&["status", "--json"])?;
    output.ok("tailscale status")?;
    let status = serde_json::from_str::<TailscaleStatus>(&output.stdout).map_err(|error| {
        AppError::Operational(format!("cannot parse tailscale status JSON: {error}"))
    })?;
    Ok(status)
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct TailscaleStatus {
    #[serde(default, rename = "BackendState")]
    pub backend_state: Option<String>,
    #[serde(default, rename = "Self")]
    pub self_node: Option<NodeInfo>,
    #[serde(default, rename = "Peer")]
    pub peer: HashMap<String, NodeInfo>,
    #[serde(default, rename = "Health")]
    pub health: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct NodeInfo {
    #[serde(default, rename = "HostName")]
    pub host_name: Option<String>,
    #[serde(default, rename = "DNSName")]
    pub dns_name: Option<String>,
    #[serde(default, rename = "Online")]
    pub online: Option<bool>,
    #[serde(default, rename = "TailscaleIPs")]
    pub tailscale_ips: Vec<String>,
    #[serde(default, rename = "KeyExpiry")]
    pub key_expiry: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = r#"{
        "BackendState": "Running",
        "Self": {
            "HostName": "smoke-node",
            "DNSName": "smoke-node.example.com.",
            "Online": true,
            "TailscaleIPs": ["100.64.0.1", "fd7a:115c:a1e0::1"],
            "KeyExpiry": "2030-01-01T00:00:00Z"
        },
        "Peer": {
            "nodekey:abc": {
                "HostName": "server",
                "DNSName": "server.example.com.",
                "Online": true,
                "TailscaleIPs": ["100.64.0.2"]
            }
        },
        "Health": []
    }"#;

    #[test]
    fn parses_status_fixture() {
        let status: TailscaleStatus = serde_json::from_str(FIXTURE).unwrap();
        assert_eq!(status.backend_state.as_deref(), Some("Running"));
        let node = status.self_node.unwrap();
        assert_eq!(node.host_name.as_deref(), Some("smoke-node"));
        assert_eq!(node.tailscale_ips.len(), 2);
        assert_eq!(status.peer.len(), 1);
        assert!(status.health.is_empty());
    }

    #[test]
    fn tolerates_unknown_fields_and_missing_keys() {
        let status: TailscaleStatus =
            serde_json::from_str(r#"{"User": {}, "Unknown": true}"#).unwrap();
        assert_eq!(status.backend_state, None);
        assert!(status.peer.is_empty());
    }
}
