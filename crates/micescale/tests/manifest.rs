use std::path::Path;

use serde::Deserialize;

const MANIFEST: &str = include_str!("../../../manifests/linuxmice-component.toml");

#[derive(Deserialize)]
struct Manifest {
    schema_version: String,
    id: String,
    #[allow(dead_code)]
    display_name: String,
    #[allow(dead_code)]
    version: String,
    #[allow(dead_code)]
    summary: String,
    component_type: String,
    standalone: bool,
    platforms: Vec<String>,
    documentation: String,
    installation: Installation,
    capabilities: Capabilities,
    interfaces: Interfaces,
    authentication: Authentication,
    data: Data,
    health: Health,
}

#[derive(Deserialize)]
struct Installation {
    package: String,
    #[serde(default)]
    service: Option<String>,
}

#[derive(Deserialize)]
struct Capabilities {
    provides: Vec<String>,
    requires: Vec<String>,
}

#[derive(Deserialize)]
struct Interfaces {
    mode: String,
    commands: Vec<String>,
    endpoints: Vec<String>,
}

#[derive(Deserialize)]
struct Authentication {
    required: Vec<String>,
    optional: Vec<String>,
    #[allow(dead_code)]
    provides: Vec<String>,
}

#[derive(Deserialize)]
struct Data {
    ownership: String,
    backup_responsibility: String,
}

#[derive(Deserialize)]
struct Health {
    kind: String,
    read_only: bool,
    timeout_ms: u64,
    command: Option<Command>,
}

#[derive(Deserialize)]
struct Command {
    executable: String,
    args: Vec<String>,
}

fn identifier(value: &str) -> bool {
    value.len() >= 3
        && value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'.' | b'-' | b'_')
        })
}

fn no_secrets(value: &str) -> bool {
    !value.to_ascii_lowercase().contains("password=")
        && !value.to_ascii_lowercase().contains("token=")
        && !value.to_ascii_lowercase().contains("secret=")
        && !value.contains('\n')
}

#[test]
fn manifest_satisfies_linuxmice_component_contract() {
    let manifest: Manifest = toml::from_str(MANIFEST).expect("manifest parses as TOML");

    assert_eq!(manifest.schema_version, "linuxmice.component.v1");
    assert!(manifest.standalone, "components must be standalone");
    assert!(identifier(&manifest.id), "id must be a valid identifier");
    assert!(identifier(&manifest.component_type));
    assert!(!manifest.platforms.is_empty());
    assert!(
        manifest.documentation.starts_with("https://"),
        "documentation must be an HTTPS URL"
    );

    assert!(identifier(&manifest.installation.package));
    assert!(
        manifest
            .installation
            .service
            .as_deref()
            .is_some_and(|name| name.ends_with(".service") && !name.contains('/'))
    );

    assert!(!manifest.capabilities.provides.is_empty());
    assert!(manifest.capabilities.requires.is_empty());
    assert!(manifest.capabilities.provides.iter().all(|c| identifier(c)));

    assert_eq!(manifest.interfaces.mode, "command");
    assert_eq!(manifest.interfaces.endpoints.len(), 0);
    assert!(!manifest.interfaces.commands.is_empty());
    assert!(manifest.interfaces.commands.iter().all(|command| {
        let executable = command.split_whitespace().next().unwrap_or_default();
        Path::new(executable).is_absolute()
    }));
    for command in &manifest.interfaces.commands {
        assert!(
            no_secrets(command),
            "interface command leaks a secret marker"
        );
    }

    assert_eq!(manifest.authentication.required.len(), 0);
    assert!(
        manifest
            .authentication
            .optional
            .iter()
            .all(|value| identifier(value)),
        "authentication identifiers must be valid"
    );

    assert!(no_secrets(&manifest.data.ownership));
    assert!(no_secrets(&manifest.data.backup_responsibility));

    assert_eq!(manifest.health.kind, "command");
    assert!(manifest.health.read_only, "health probes must be read-only");
    assert!((50..=5_000).contains(&manifest.health.timeout_ms));
    let command = manifest.health.command.expect("command health probe");
    assert!(Path::new(&command.executable).is_absolute());
    assert!(!command.args.is_empty());
    assert!(no_secrets(&command.executable));
    assert!(command.args.iter().all(|arg| no_secrets(arg)));
}
