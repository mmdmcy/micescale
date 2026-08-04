use micescale_core::config;

use crate::AppError;

pub fn show(format: &str) -> Result<(), AppError> {
    let path = config::default_path();
    let Some(config) = config::load(&path)? else {
        return Err(AppError::Operational(format!(
            "no config at {}; run `micescale enroll` first",
            path.display()
        )));
    };
    match format {
        "json" => println!(
            "{}",
            serde_json::to_string_pretty(&config).expect("serializes")
        ),
        "yaml" => println!("{}", serde_yaml_ng::to_string(&config).expect("serializes")),
        _ => {
            println!("control_server: {}", config.control_server);
            println!("carrier: {}", config.carrier);
            println!(
                "node_name: {}",
                config.node_name.as_deref().unwrap_or("(hostname)")
            );
            println!("audit_log: {}", config.audit_path().display());
        }
    }
    Ok(())
}
