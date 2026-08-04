use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

const HUB_PRIV: &str = "aHViLXByaXZhdGUta2V5LWh1Yi1wcml2YXRlLWtleS0=";
const HUB_PUB: &str = "aHViLXB1YmxpYy1rZXktaHViLXB1YmxpYy1rZXktaHU=";
const CLIENT_PRIV: &str = "Y2xpZW50LXByaXZhdGUta2V5LWNsaWVudC1wcml2YXQ=";
const CLIENT_PUB: &str = "Y2xpZW50LXB1YmxpYy1rZXktY2xpZW50LXB1YmxpYy0=";

fn binary() -> PathBuf {
    env!("CARGO_BIN_EXE_micescale").into()
}

fn write_executable(path: &Path, content: &str) {
    fs::write(path, content).unwrap();
    let mut permissions = fs::metadata(path).unwrap().permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(path, permissions).unwrap();
}

fn fake_binaries(dir: &Path, private: &str, public: &str, dump: &str) -> (String, String) {
    fs::write(dir.join("dump.txt"), dump).unwrap();
    write_executable(
        &dir.join("fake-wg"),
        &format!(
            "#!/bin/sh\ncase \"$1\" in\n  genkey) echo \"{private}\" ;;\n  pubkey) cat >/dev/null; echo \"{public}\" ;;\n  show) cat \"{dump_path}\" ;;\n  version) echo \"wireguard-tools v1.0\" ;;\n  *) echo \"unexpected: $*\" >&2; exit 2 ;;\nesac\n",
            dump_path = dir.join("dump.txt").display()
        ),
    );
    write_executable(
        &dir.join("fake-wg-quick"),
        "#!/bin/sh\necho \"wg-quick: $*\" >> \"$FAKE_WGQUICK_LOG\"\n",
    );
    (
        dir.join("fake-wg").to_string_lossy().into_owned(),
        dir.join("fake-wg-quick").to_string_lossy().into_owned(),
    )
}

struct Env {
    _dir: tempfile::TempDir,
    root: PathBuf,
    wg_bin: String,
    wgquick_bin: String,
}

fn setup(private: &str, public: &str, dump: &str) -> Env {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().to_path_buf();
    let (wg_bin, wgquick_bin) = fake_binaries(&root, private, public, dump);
    Env {
        _dir: dir,
        root,
        wg_bin,
        wgquick_bin,
    }
}

impl Env {
    fn run(&self, args: &[&str]) -> (i32, String, String) {
        let output = Command::new(binary())
            .args(args)
            .env("MICESCALE_CONFIG", self.root.join("config.toml"))
            .env("MICESCALE_AUDIT_LOG", self.root.join("audit.jsonl"))
            .env("MICESCALE_WG_DIR", self.root.join("wg"))
            .env("MICESCALE_WG_BIN", &self.wg_bin)
            .env("MICESCALE_WGQUICK_BIN", &self.wgquick_bin)
            .env("FAKE_WGQUICK_LOG", self.root.join("wg-quick.log"))
            .output()
            .unwrap();
        (
            output.status.code().unwrap_or(-1),
            String::from_utf8_lossy(&output.stdout).into_owned(),
            String::from_utf8_lossy(&output.stderr).into_owned(),
        )
    }

    fn file(&self, name: &str) -> String {
        fs::read_to_string(self.root.join("wg").join(name)).unwrap()
    }
}

const HUB_DUMP: &str = "lmice0\tDUMPPUB\tK3k=\t51820\toff\nlmice0\tDUMPPUB\t\t10.60.0.2:4\t10.60.0.2/32\t1780000000\t1234\t5678\t25\n";

#[test]
fn hub_lifecycle_without_secrets_in_registry() {
    let env = setup(HUB_PRIV, HUB_PUB, HUB_DUMP);
    let (code, stdout, stderr) = env.run(&[
        "wg",
        "hub-init",
        "--address",
        "10.60.0.1/24",
        "--endpoint",
        "hub.example.com:51820",
    ]);
    assert_eq!(code, 0, "hub-init failed: {stderr}");
    assert!(
        stdout.contains(&format!("public key : {HUB_PUB}")),
        "{stdout}"
    );

    let (code, _, stderr) = env.run(&[
        "wg",
        "hub-add-peer",
        "--name",
        "laptop",
        "--pubkey",
        CLIENT_PUB,
        "--address",
        "10.60.0.2",
    ]);
    assert_eq!(code, 0, "hub-add-peer failed: {stderr}");

    let (code, _, stderr) = env.run(&["wg", "hub-render"]);
    assert_eq!(code, 0, "hub-render failed: {stderr}");

    let hub_conf = env.file("hub.conf");
    assert!(
        hub_conf.contains(HUB_PRIV),
        "hub.conf must carry the private key"
    );
    assert!(
        hub_conf.contains(&format!("PublicKey = {CLIENT_PUB}")),
        "{hub_conf}"
    );

    let hub_json = env.file("hub.json");
    assert!(
        !hub_json.contains(HUB_PRIV),
        "hub.json must not carry the private key"
    );
    assert!(
        hub_json.contains(&format!("\"pubkey\": \"{HUB_PUB}\"")),
        "{hub_json}"
    );

    let audit = fs::read_to_string(env.root.join("audit.jsonl")).unwrap();
    assert!(audit.contains("wg-hub-init"), "{audit}");
    assert!(audit.contains("wg-hub-add-peer"), "{audit}");
    assert!(!audit.contains(HUB_PRIV), "audit leaked the private key");

    let (code, stdout, _) = env.run(&["wg", "status", "--format", "json"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("\"side\": \"hub\""), "{stdout}");
    assert!(stdout.contains("\"name\": \"laptop\""), "{stdout}");

    let (code, _, stderr) = env.run(&["wg", "up"]);
    assert_eq!(code, 0, "wg up failed: {stderr}");
    let (code, _, _) = env.run(&["wg", "down"]);
    assert_eq!(code, 0);
}

#[test]
fn client_lifecycle_and_leave() {
    let dump = format!(
        "lmice0\tDUMPPUB\tK3k=\t51820\toff\nlmice0\t{HUB_PUB}\t\t10.60.0.1:51820\t10.60.0.0/24\t1780000000\t99\t42\t25\n"
    );
    let env = setup(CLIENT_PRIV, CLIENT_PUB, &dump);
    let (code, stdout, stderr) = env.run(&[
        "wg",
        "client-init",
        "--address",
        "10.60.0.2/24",
        "--endpoint",
        "hub.example.com:51820",
        "--hub-pubkey",
        HUB_PUB,
        "--allowed-ips",
        "10.60.0.0/24",
    ]);
    assert_eq!(code, 0, "client-init failed: {stderr}");
    assert!(
        stdout.contains(&format!("public key : {CLIENT_PUB}")),
        "{stdout}"
    );

    let client_conf = env.file("client.conf");
    assert!(client_conf.contains(CLIENT_PRIV));
    assert!(client_conf.contains("PersistentKeepalive = 25"));

    let client_json = env.file("client.json");
    assert!(
        !client_json.contains(CLIENT_PRIV),
        "client.json leaked the key"
    );

    let (code, stdout, _) = env.run(&["config", "show", "--format", "json"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("\"carrier\": \"wireguard\""), "{stdout}");

    let (code, stdout, _) = env.run(&["wg", "status", "--format", "json"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("\"side\": \"client\""), "{stdout}");
    assert!(stdout.contains("\"running\": true"), "{stdout}");
    assert!(stdout.contains("\"name\": \"hub\""), "{stdout}");

    let (code, stdout, _) = env.run(&["doctor", "--format", "json"]);
    assert_eq!(
        code, 0,
        "doctor should pass with fake binaries up: {stdout}"
    );
    assert!(stdout.contains("\"carrier\": \"wireguard\""), "{stdout}");

    let (code, _, stderr) = env.run(&["leave"]);
    assert_eq!(code, 0, "leave failed: {stderr}");
    assert!(
        !env.root.join("wg").join("client.json").exists(),
        "leave must remove client state"
    );
    let audit = fs::read_to_string(env.root.join("audit.jsonl")).unwrap();
    assert!(audit.contains("wg-leave"), "{audit}");

    let (code, _, stderr) = env.run(&["wg", "status", "--format", "json"]);
    assert_eq!(code, 1);
    assert!(stderr.contains("no wireguard state"), "{stderr}");
}

#[test]
fn enroll_rejects_wireguard_carrier_with_guidance() {
    let env = setup(HUB_PRIV, HUB_PUB, HUB_DUMP);
    let (code, _, stderr) = env.run(&[
        "enroll",
        "--server",
        "https://headscale.example.com",
        "--authkey",
        "x",
        "--carrier",
        "wireguard",
    ]);
    assert_eq!(code, 2, "usage error expected");
    assert!(stderr.contains("wg client-init"), "{stderr}");
}
