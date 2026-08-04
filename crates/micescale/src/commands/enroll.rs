use std::time::{SystemTime, UNIX_EPOCH};

use micescale_core::audit;
use micescale_core::config::{self, Config};

use crate::AppError;
use crate::tailscale;

pub fn run(
    server: &str,
    authkey: &str,
    node_name: Option<&str>,
    carrier: &str,
) -> Result<(), AppError> {
    if carrier == "wireguard" {
        return Err(AppError::Usage(
            "the wireguard carrier has no coordination server; run `micescale wg client-init` instead"
                .into(),
        ));
    }
    if authkey.trim().is_empty() {
        return Err(AppError::Usage(
            "an auth key is required via --authkey or MICESCALE_AUTHKEY".into(),
        ));
    }
    let config = Config::new(
        carrier.to_string(),
        Some(server.to_string()),
        node_name.map(str::to_string),
    );
    config::validate(&config)?;

    tailscale::up(server, node_name, authkey)?;

    let path = config::default_path();
    config::save(&path, &config)?;

    append_event(
        &config,
        "enroll",
        "ok",
        Some("joined via pre-auth key; key never persisted"),
    )?;

    println!(
        "enrolled {} into {carrier} at {server}",
        node_name.unwrap_or("node")
    );
    println!("config written to {}", path.display());
    Ok(())
}

pub(crate) fn append_event(
    config: &Config,
    event: &str,
    status: &str,
    detail: Option<&str>,
) -> Result<(), AppError> {
    let path = config.audit_path();
    let parent = path.parent().ok_or_else(|| {
        AppError::Operational(format!("audit path {} has no parent", path.display()))
    })?;
    std::fs::create_dir_all(parent).map_err(|error| {
        AppError::Operational(format!("cannot create {}: {error}", parent.display()))
    })?;
    let event = audit::AuditEvent {
        ts: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock after epoch")
            .as_secs(),
        event: event.to_string(),
        carrier: config.carrier.clone(),
        control_server: config.control_server.clone(),
        status: status.to_string(),
        node_name: config.node_name.clone(),
        detail: detail.map(str::to_string),
    };
    audit::append(&path, &event)?;
    Ok(())
}
