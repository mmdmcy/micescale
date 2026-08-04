use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

const FIXTURE_STATUS: &str = r#"{"BackendState":"Running","Self":{"HostName":"smoke-node","DNSName":"smoke-node.example.com.","Online":true,"TailscaleIPs":["100.64.0.1"]},"Peer":{"a":{"Online":true}},"Health":[]}"#;

fn binary() -> PathBuf {
    env!("CARGO_BIN_EXE_micescale").into()
}

fn fake_tailscale(dir: &std::path::Path) -> String {
    let fixture = dir.join("status-fixture.json");
    fs::write(&fixture, FIXTURE_STATUS).unwrap();
    let script = dir.join("fake-tailscale");
    fs::write(
        &script,
        format!(
            r#"#!/bin/sh
case "$1" in
  status) cat "{FIXTURE}"; exit 0 ;;
  up|logout) echo "fake: $*" >> "$FAKE_LOG"; exit 0 ;;
  *) echo "unexpected: $*" >&2; exit 2 ;;
esac
"#,
            FIXTURE = fixture.display()
        ),
    )
    .unwrap();
    let mut permissions = fs::metadata(&script).unwrap().permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&script, permissions).unwrap();
    script.to_string_lossy().into_owned()
}

fn run_with(dir: &Path, args: &[&str], tailscale_bin: &str) -> (i32, String, String) {
    let output = Command::new(binary())
        .args(args)
        .env("MICESCALE_CONFIG", dir.join("config.toml"))
        .env("MICESCALE_AUDIT_LOG", dir.join("audit.jsonl"))
        .env("MICESCALE_TAILSCALE_BIN", tailscale_bin)
        .output()
        .unwrap();
    (
        output.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

#[test]
fn full_enroll_status_leave_cycle_without_secrets() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let tailscale_bin = fake_tailscale(root);

    let (code, _, stderr) = run_with(
        root,
        &[
            "enroll",
            "--server",
            "https://headscale.example.com",
            "--authkey",
            "top-secret-key",
            "--node-name",
            "smoke-node",
        ],
        &tailscale_bin,
    );
    assert_eq!(code, 0, "enroll failed: {stderr}");

    let (code, stdout, stderr) = run_with(root, &["status", "--format", "json"], &tailscale_bin);
    assert_eq!(code, 0, "status failed: {stderr}");
    assert!(stdout.contains("carrier-untrusted"), "{stdout}");
    assert!(stdout.contains("\"online_peers\": 1"), "{stdout}");

    let (code, _, stderr) = run_with(root, &["leave"], &tailscale_bin);
    assert_eq!(code, 0, "leave failed: {stderr}");

    let (code, stdout, stderr) =
        run_with(root, &["audit", "tail", "--format", "json"], &tailscale_bin);
    assert_eq!(code, 0, "audit failed: {stderr}");
    assert!(stdout.contains("enroll"), "{stdout}");
    assert!(stdout.contains("leave"), "{stdout}");

    let config = fs::read_to_string(root.join("config.toml")).unwrap();
    let audit = fs::read_to_string(root.join("audit.jsonl")).unwrap();
    assert!(!config.contains("top-secret-key"), "config leaked the key");
    assert!(!audit.contains("top-secret-key"), "audit leaked the key");
}

#[test]
fn status_without_config_fails_cleanly() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let (code, _, stderr) = run_with(root, &["status"], "/bin/false");
    assert_eq!(code, 1);
    assert!(stderr.contains("micescale enroll"), "{stderr}");
}

#[test]
fn carrier_policy_is_carrier_untrusted() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let (code, stdout, stderr) = run_with(
        root,
        &["carrier", "policy", "--format", "json"],
        "/bin/false",
    );
    assert_eq!(code, 0, "{stderr}");
    assert!(
        stdout.contains("\"posture\": \"carrier-untrusted\""),
        "{stdout}"
    );
}
