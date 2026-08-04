use serde::Serialize;

use micescale_core::config;

use crate::AppError;
use crate::tailscale::{self, NodeInfo, TailscaleStatus};

#[derive(Debug, Serialize)]
pub struct StatusReport {
    pub product: &'static str,
    pub posture: &'static str,
    pub carrier: String,
    pub control_server: String,
    pub backend_state: Option<String>,
    pub node: Option<SelfNode>,
    pub online_peers: usize,
    pub total_peers: usize,
    pub health_warnings: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct SelfNode {
    pub hostname: Option<String>,
    pub dns_name: Option<String>,
    pub online: Option<bool>,
    pub tailscale_ips: Vec<String>,
    pub key_expiry: Option<String>,
}

pub fn run(format: &str) -> Result<(), AppError> {
    let path = config::default_path();
    let Some(config) = config::load(&path)? else {
        return Err(AppError::Operational(format!(
            "no config at {}; run `micescale enroll` or `micescale wg client-init` first",
            path.display()
        )));
    };
    if config.carrier == "wireguard" {
        return crate::commands::wg::status(format);
    }
    let raw = tailscale::status_json()?;
    let report = build(
        config.control_server.as_deref().unwrap_or("(unknown)"),
        &config.carrier,
        &raw,
    );
    match format {
        "json" => println!(
            "{}",
            serde_json::to_string_pretty(&report).expect("serializes")
        ),
        "yaml" => println!("{}", serde_yaml_ng::to_string(&report).expect("serializes")),
        _ => {
            println!("posture: {}", report.posture);
            println!("carrier: {} at {}", report.carrier, report.control_server);
            println!(
                "backend_state: {}",
                report.backend_state.as_deref().unwrap_or("unknown")
            );
            if let Some(node) = &report.node {
                println!(
                    "node: {} ({})",
                    node.hostname.as_deref().unwrap_or("unknown"),
                    node.dns_name.as_deref().unwrap_or("unknown")
                );
                println!("online: {}", node.online.unwrap_or(false));
                println!("addresses: {}", node.tailscale_ips.join(", "));
            }
            println!(
                "peers: {}/{} online",
                report.online_peers, report.total_peers
            );
            for warning in &report.health_warnings {
                println!("warning: {warning}");
            }
        }
    }
    Ok(())
}

fn build(control_server: &str, carrier: &str, raw: &TailscaleStatus) -> StatusReport {
    let online_peers = raw
        .peer
        .values()
        .filter(|peer| peer.online == Some(true))
        .count();
    StatusReport {
        product: micescale_core::PRODUCT,
        posture: micescale_core::carrier::POSTURE,
        carrier: carrier.to_string(),
        control_server: control_server.to_string(),
        backend_state: raw.backend_state.clone(),
        node: raw.self_node.as_ref().map(node_info),
        online_peers,
        total_peers: raw.peer.len(),
        health_warnings: raw.health.clone(),
    }
}

fn node_info(info: &NodeInfo) -> SelfNode {
    SelfNode {
        hostname: info.host_name.clone(),
        dns_name: info.dns_name.clone(),
        online: info.online,
        tailscale_ips: info.tailscale_ips.clone(),
        key_expiry: info.key_expiry.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> TailscaleStatus {
        serde_json::from_str(
            r#"{
                "BackendState": "Running",
                "Self": {"HostName": "smoke-node", "Online": true, "TailscaleIPs": ["100.64.0.1"]},
                "Peer": {
                    "a": {"Online": true},
                    "b": {"Online": false}
                },
                "Health": ["something to look at"]
            }"#,
        )
        .unwrap()
    }

    #[test]
    fn report_counts_peers_and_keeps_posture() {
        let report = build("https://headscale.example.com", "headscale", &fixture());
        assert_eq!(report.posture, "carrier-untrusted");
        assert_eq!(report.online_peers, 1);
        assert_eq!(report.total_peers, 2);
        assert_eq!(report.health_warnings.len(), 1);
    }
}
