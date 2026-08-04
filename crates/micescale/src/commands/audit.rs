use micescale_core::audit;
use micescale_core::config;

use crate::AppError;

pub fn tail(limit: usize, format: &str) -> Result<(), AppError> {
    let path = config::default_path();
    let audit_path = match config::load(&path)? {
        Some(config) => config.audit_path(),
        None => micescale_core::paths::default_audit_path(),
    };
    let events = audit::tail(&audit_path, limit)?;
    match format {
        "json" => println!(
            "{}",
            serde_json::to_string_pretty(&events).expect("serializes")
        ),
        "yaml" => println!("{}", serde_yaml_ng::to_string(&events).expect("serializes")),
        _ => {
            for event in events {
                println!(
                    "{:<10} {:<10} {:<10} {} ({})",
                    event.ts,
                    event.event,
                    event.status,
                    event.control_server,
                    event.node_name.as_deref().unwrap_or("-")
                );
            }
        }
    }
    Ok(())
}
