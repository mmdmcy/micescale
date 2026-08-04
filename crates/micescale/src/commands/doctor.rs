use micescale_core::config;
use micescale_core::health::{HealthCheck, HealthReport};

use crate::AppError;
use crate::tailscale;

pub fn run(peer: Option<&str>, format: &str) -> Result<(), AppError> {
    let path = config::default_path();
    let config = config::load(&path)?.ok_or_else(|| {
        AppError::Operational(format!(
            "no config at {}; run `micescale enroll` first",
            path.display()
        ))
    })?;

    let mut report = HealthReport::new(config.carrier.clone());

    report.add(HealthCheck {
        name: "tailscale-binary".into(),
        ok: tailscale::binary().is_ok(),
        detail: Some("tailscale client must be installed".into()),
    });

    let status = match tailscale::status_json() {
        Ok(status) => {
            report.add(HealthCheck {
                name: "tailscaled".into(),
                ok: true,
                detail: Some(format!(
                    "backend state: {}",
                    status.backend_state.as_deref().unwrap_or("unknown")
                )),
            });
            Some(status)
        }
        Err(error) => {
            report.add(HealthCheck {
                name: "tailscaled".into(),
                ok: false,
                detail: Some(error.to_string()),
            });
            None
        }
    };

    report.add(control_server_check(&config.control_server));

    let wireguard_ok = std::fs::metadata("/sys/module/wireguard").is_ok();
    report.add(HealthCheck {
        name: "wireguard-kernel".into(),
        ok: wireguard_ok,
        detail: Some(if wireguard_ok {
            "wireguard kernel module present".into()
        } else {
            "wireguard kernel module missing; userspace networking may still work".into()
        }),
    });

    if let Some(peer) = peer {
        let found = status.and_then(|status| {
            status
                .peer
                .values()
                .find(|info| {
                    info.host_name.as_deref() == Some(peer)
                        || info.dns_name.as_deref() == Some(peer)
                        || info.tailscale_ips.iter().any(|ip| ip == peer)
                })
                .cloned()
        });
        match found {
            Some(info) => {
                let online = info.online == Some(true);
                report.add(HealthCheck {
                    name: format!("peer:{peer}"),
                    ok: online,
                    detail: Some(if online {
                        "peer reachable".into()
                    } else {
                        "peer present but offline".into()
                    }),
                });
            }
            None => {
                report.add(HealthCheck {
                    name: format!("peer:{peer}"),
                    ok: false,
                    detail: Some("peer not found in tailnet".into()),
                });
            }
        }
    }

    match format {
        "json" => println!(
            "{}",
            serde_json::to_string_pretty(&report).expect("serializes")
        ),
        "yaml" => println!("{}", serde_yaml_ng::to_string(&report).expect("serializes")),
        _ => {
            for check in &report.checks {
                println!(
                    "{:<20} {}  {}",
                    check.name,
                    if check.ok { "ok" } else { "FAIL" },
                    check.detail.as_deref().unwrap_or("")
                );
            }
        }
    }

    if report.healthy {
        Ok(())
    } else {
        Err(AppError::Operational(
            "one or more health checks failed".into(),
        ))
    }
}

fn control_server_check(server: &str) -> HealthCheck {
    let endpoint = format!("{server}/health");
    match ureq::get(&endpoint)
        .config()
        .timeout_global(Some(std::time::Duration::from_secs(5)))
        .build()
        .call()
    {
        Ok(_) => HealthCheck {
            name: "control-server".into(),
            ok: true,
            detail: Some(format!("{endpoint} reachable")),
        },
        Err(error) => HealthCheck {
            name: "control-server".into(),
            ok: false,
            detail: Some(format!("{endpoint} unreachable: {error}")),
        },
    }
}
