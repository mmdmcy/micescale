use std::collections::HashMap;
use std::env;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use micescale_core::paths::TAILSCALE_BIN_ENV;

use crate::AppError;
use crate::files;
use crate::process;

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

/// `tailscale up` against a self-hosted control server. The auth key goes
/// through a 0600 file (`--auth-key=file:PATH`) so it never shows up in argv.
pub fn up(
    server: &str,
    node_name: Option<&str>,
    authkey: &str,
    force_reauth: bool,
) -> Result<(), AppError> {
    let key_path = env::temp_dir().join(format!("micescale-authkey-{}", std::process::id()));
    files::write_private(&key_path, authkey)?;
    let result = run_up(&up_args(server, node_name, &key_path, force_reauth));
    let _ = std::fs::remove_file(&key_path);
    result
}

fn run_up(args: &[String]) -> Result<(), AppError> {
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    process::run(&binary()?, &args)?.ok("tailscale up")
}

fn up_args(
    server: &str,
    node_name: Option<&str>,
    key_path: &Path,
    force_reauth: bool,
) -> Vec<String> {
    let mut args = vec![
        "up".to_string(),
        "--login-server".into(),
        server.into(),
        format!("--auth-key=file:{}", key_path.display()),
    ];
    // Only needed when switching a device from another control server (for
    // example Tailscale cloud); it drops the current login, so it is opt-in.
    if force_reauth {
        args.push("--force-reauth".into());
    }
    if let Some(name) = node_name {
        args.push("--hostname".into());
        args.push(name.into());
    }
    args
}

pub fn logout() -> Result<(), AppError> {
    process::run(&binary()?, &["logout"])?.ok("tailscale logout")
}

pub fn status_json() -> Result<TailscaleStatus, AppError> {
    let output = process::run(&binary()?, &["status", "--json"])?;
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
    fn up_passes_the_key_by_file_and_reauths_only_on_request() {
        let path = Path::new("/tmp/key");
        let args = up_args("https://hs.example.com", Some("box"), path, false);
        assert!(args.contains(&"--auth-key=file:/tmp/key".to_string()));
        assert!(!args.iter().any(|arg| arg == "--authkey"));
        assert!(!args.iter().any(|arg| arg == "--force-reauth"));
        let args = up_args("https://hs.example.com", None, path, true);
        assert!(args.iter().any(|arg| arg == "--force-reauth"));
        assert!(!args.iter().any(|arg| arg == "--hostname"));
    }

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
