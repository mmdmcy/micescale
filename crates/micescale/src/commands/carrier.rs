use micescale_core::carrier;

use crate::AppError;

pub fn policy(format: &str) -> Result<(), AppError> {
    let policy = carrier::carrier_policy();
    match format {
        "json" => println!(
            "{}",
            serde_json::to_string_pretty(&policy).expect("serializes")
        ),
        "yaml" => println!("{}", serde_yaml_ng::to_string(&policy).expect("serializes")),
        _ => {
            println!("product: {}", policy.product);
            println!("posture: {}", policy.posture);
            println!("authorization rule: {}", policy.authorization_rule);
            println!("encryption rule: {}", policy.encryption_rule);
            println!("metadata warning: {}", policy.metadata_warning);
            println!("enterprise requirements:");
            for requirement in &policy.enterprise_requirements {
                println!("  - {requirement}");
            }
        }
    }
    Ok(())
}

pub fn profiles(format: &str) -> Result<(), AppError> {
    let profiles = carrier::carrier_profiles();
    match format {
        "json" => println!(
            "{}",
            serde_json::to_string_pretty(&profiles).expect("serializes")
        ),
        "yaml" => println!(
            "{}",
            serde_yaml_ng::to_string(&profiles).expect("serializes")
        ),
        _ => {
            for profile in &profiles {
                println!(
                    "{:<12} {:<10} carrier={:<40} enterprise_default={}",
                    profile.id, profile.status, profile.carrier, profile.enterprise_default
                );
            }
        }
    }
    Ok(())
}
