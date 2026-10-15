use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

const HUB_PRIV: &str = "aHViLXByaXZhdGUta2V5LWh1Yi1wcml2YXRlLWtleS0=";
const HUB_PUB: &str = "aHViLXB1YmxpYy1rZXktaHViLXB1YmxpYy1rZXktaHU=";
const CLIENT_PRIV: &str = "Y2xpZW50LXByaXZhdGUta2V5LWNsaWVudC1wcml2YXQ=";
const CLIENT_PUB: &str = "Y2xpZW50LXB1YmxpYy1rZXktY2xpZW50LXB1YmxpYy0=";
const SECOND_PUB: &str = "c2Vjb25kLXB1YmxpYy1rZXktc2Vjb25kLXB1YmxpYy0=";

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

    let hub_conf = env.file("lmice0.conf");
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

    let client_conf = env.file("lmice0.conf");
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

#[test]
fn hub_remove_peer_and_duplicate_rejection() {
    let env = setup(HUB_PRIV, HUB_PUB, HUB_DUMP);
    for args in [
        vec![
            "wg",
            "hub-init",
            "--address",
            "10.60.0.1/24",
            "--endpoint",
            "hub.example.com:51820",
        ],
        vec![
            "wg",
            "hub-add-peer",
            "--name",
            "laptop",
            "--pubkey",
            CLIENT_PUB,
            "--address",
            "10.60.0.2",
        ],
        vec![
            "wg",
            "hub-add-peer",
            "--name",
            "server",
            "--pubkey",
            SECOND_PUB,
            "--address",
            "10.60.0.3",
        ],
    ] {
        let (code, _, stderr) = env.run(&args);
        assert_eq!(code, 0, "step failed: {stderr}");
    }

    let (code, _, stderr) = env.run(&[
        "wg",
        "hub-add-peer",
        "--name",
        "laptop",
        "--pubkey",
        SECOND_PUB,
        "--address",
        "10.60.0.9",
    ]);
    assert_eq!(code, 1, "duplicate name must be rejected");
    assert!(stderr.contains("already exists"), "{stderr}");

    let (code, _, stderr) = env.run(&[
        "wg",
        "hub-add-peer",
        "--name",
        "laptop2",
        "--pubkey",
        CLIENT_PUB,
        "--address",
        "10.60.0.4",
    ]);
    assert_eq!(code, 1, "duplicate pubkey must be rejected");
    assert!(stderr.contains("already exists"), "{stderr}");

    let (code, _, stderr) = env.run(&[
        "wg",
        "hub-add-peer",
        "--name",
        "laptop3",
        "--pubkey",
        SECOND_PUB,
        "--address",
        "10.60.0.2",
    ]);
    assert_eq!(code, 1, "duplicate address must be rejected");
    assert!(stderr.contains("already exists"), "{stderr}");

    let (code, _, stderr) = env.run(&["wg", "hub-remove-peer", "--name", "laptop"]);
    assert_eq!(code, 0, "hub-remove-peer failed: {stderr}");

    let hub_conf = env.file("lmice0.conf");
    assert!(
        !hub_conf.contains(CLIENT_PUB),
        "removed peer must leave hub.conf"
    );
    assert!(hub_conf.contains(SECOND_PUB), "{hub_conf}");

    let audit = fs::read_to_string(env.root.join("audit.jsonl")).unwrap();
    assert!(audit.contains("wg-hub-remove-peer"), "{audit}");
    assert!(!audit.contains(HUB_PRIV), "audit leaked the private key");

    let (code, _, stderr) = env.run(&["wg", "hub-remove-peer", "--name", "ghost"]);
    assert_eq!(code, 1, "removing a missing peer must fail");
    assert!(stderr.contains("no peer named ghost"), "{stderr}");
}

#[test]
fn validation_rejects_bad_inputs() {
    let env = setup(HUB_PRIV, HUB_PUB, HUB_DUMP);

    let (code, _, stderr) = env.run(&[
        "wg",
        "hub-init",
        "--address",
        "10.60.0.1/24",
        "--endpoint",
        "hub.example.com",
    ]);
    assert_eq!(code, 2);
    assert!(stderr.contains("host:port"), "{stderr}");

    let (code, _, stderr) = env.run(&[
        "wg",
        "hub-init",
        "--address",
        "10.60.0.1/24",
        "--endpoint",
        "hub.example.com:0",
    ]);
    assert_eq!(code, 2);
    assert!(stderr.contains("must not be 0"), "{stderr}");

    let (code, _, stderr) = env.run(&[
        "wg",
        "hub-init",
        "--address",
        "10.60.0.1/24",
        "--endpoint",
        "hub.example.com:notaport",
    ]);
    assert_eq!(code, 2);
    assert!(stderr.contains("port must be a number"), "{stderr}");

    let (code, _, stderr) = env.run(&[
        "wg",
        "hub-init",
        "--address",
        "10.60.0.1",
        "--endpoint",
        "hub.example.com:51820",
    ]);
    assert_eq!(code, 2);
    assert!(stderr.contains("prefix length"), "{stderr}");

    let (code, _, stderr) = env.run(&[
        "wg",
        "hub-init",
        "--listen-port",
        "0",
        "--address",
        "10.60.0.1/24",
        "--endpoint",
        "hub.example.com:51820",
    ]);
    assert_eq!(code, 2);
    assert!(stderr.contains("listen-port must not be 0"), "{stderr}");

    let (code, _, stderr) = env.run(&[
        "wg",
        "hub-add-peer",
        "--name",
        "",
        "--pubkey",
        CLIENT_PUB,
        "--address",
        "10.60.0.2",
    ]);
    assert_eq!(code, 2);
    assert!(stderr.contains("must not be empty"), "{stderr}");

    let (code, _, stderr) = env.run(&[
        "wg",
        "hub-add-peer",
        "--name",
        "x",
        "--pubkey",
        "not-a-key",
        "--address",
        "10.60.0.2",
    ]);
    assert_eq!(code, 2);
    assert!(
        stderr.contains("not a valid WireGuard public key"),
        "{stderr}"
    );

    let (code, _, stderr) = env.run(&[
        "wg",
        "hub-add-peer",
        "--name",
        "x",
        "--pubkey",
        CLIENT_PUB,
        "--address",
        "10.60.0.2/32",
    ]);
    assert_eq!(code, 2);
    assert!(stderr.contains("plain IP"), "{stderr}");

    let (code, _, stderr) = env.run(&[
        "wg",
        "client-init",
        "--address",
        "10.60.0.2/24",
        "--endpoint",
        "hub.example.com:51820",
        "--hub-pubkey",
        "bogus",
        "--allowed-ips",
        "10.60.0.0/24",
    ]);
    assert_eq!(code, 2);
    assert!(stderr.contains("hub-pubkey is not a valid"), "{stderr}");

    let (code, _, stderr) = env.run(&[
        "wg",
        "client-init",
        "--address",
        "10.60.0.2/24",
        "--endpoint",
        "hub.example.com:51820",
        "--hub-pubkey",
        HUB_PUB,
        "--allowed-ips",
        " ",
    ]);
    assert_eq!(code, 2);
    assert!(stderr.contains("allowed-ips must not be empty"), "{stderr}");
}

#[test]
fn wireguard_commands_require_state() {
    let env = setup(HUB_PRIV, HUB_PUB, HUB_DUMP);

    let (code, _, stderr) = env.run(&["wg", "up"]);
    assert_eq!(code, 1);
    assert!(stderr.contains("no wireguard state"), "{stderr}");

    let (code, _, stderr) = env.run(&["wg", "status", "--format", "json"]);
    assert_eq!(code, 1);
    assert!(stderr.contains("no wireguard state"), "{stderr}");
}

const HUB_INIT: &[&str] = &[
    "wg",
    "hub-init",
    "--address",
    "10.60.0.1/24",
    "--endpoint",
    "hub.example.com:51820",
];

fn add_peer(env: &Env, name: &str, pubkey: &str, address: &str) -> (i32, String, String) {
    env.run(&[
        "wg",
        "hub-add-peer",
        "--name",
        name,
        "--pubkey",
        pubkey,
        "--address",
        address,
    ])
}

#[test]
fn config_file_is_named_after_the_interface() {
    let dump = "wg7\tDUMPPUB\tK3k=\t51820\toff\n";
    let env = setup(HUB_PRIV, HUB_PUB, dump);
    let mut args = HUB_INIT.to_vec();
    args.extend(["--interface", "wg7"]);
    let (code, _, stderr) = env.run(&args);
    assert_eq!(code, 0, "{stderr}");
    assert!(env.file("wg7.conf").contains("ListenPort = 51820"));
    assert!(!env.root.join("wg").join("hub.conf").exists());

    let (code, _, stderr) = env.run(&["wg", "up"]);
    assert_eq!(code, 0, "{stderr}");
    let log = fs::read_to_string(env.root.join("wg-quick.log")).unwrap();
    let conf = env.root.join("wg").join("wg7.conf");
    assert!(
        log.contains(&format!("up {}", conf.display())),
        "wg-quick must get <iface>.conf so its interface is wg7: {log}"
    );

    // status/doctor look the interface up by the same name wg-quick used.
    let (code, stdout, _) = env.run(&["wg", "status", "--format", "json"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("\"interface\": \"wg7\""), "{stdout}");
    assert!(stdout.contains("\"running\": true"), "{stdout}");
    let (code, stdout, _) = env.run(&["doctor", "--format", "json"]);
    assert_eq!(code, 0, "{stdout}");

    let mut args = HUB_INIT.to_vec();
    args.extend(["--interface", "../evil", "--force"]);
    let (code, _, stderr) = env.run(&args);
    assert_eq!(code, 2);
    assert!(stderr.contains("interface must be"), "{stderr}");
}

#[test]
fn hub_add_peer_prints_a_working_syncconf_and_forwards() {
    let env = setup(HUB_PRIV, HUB_PUB, HUB_DUMP);
    assert_eq!(env.run(HUB_INIT).0, 0);
    let (code, stdout, stderr) = add_peer(&env, "laptop", CLIENT_PUB, "10.60.0.2");
    assert_eq!(code, 0, "{stderr}");
    let conf = env.root.join("wg").join("lmice0.conf");
    assert!(
        stdout.contains(&format!(
            "sudo bash -c 'wg syncconf lmice0 <(wg-quick strip {})'",
            conf.display()
        )),
        "{stdout}"
    );
    let hub_conf = env.file("lmice0.conf");
    assert!(
        hub_conf.contains(
            "PostUp = sysctl -w net.ipv4.ip_forward=1; iptables -I FORWARD -i %i -o %i -j ACCEPT"
        ),
        "{hub_conf}"
    );
    assert!(
        hub_conf.contains("PostDown = iptables -D FORWARD -i %i -o %i -j ACCEPT"),
        "{hub_conf}"
    );
    let mode = fs::metadata(&conf).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, 0o600);
}

#[test]
fn hub_init_refuses_to_clobber_without_force() {
    let env = setup(HUB_PRIV, HUB_PUB, HUB_DUMP);
    assert_eq!(env.run(HUB_INIT).0, 0);
    assert_eq!(add_peer(&env, "laptop", CLIENT_PUB, "10.60.0.2").0, 0);

    let (code, _, stderr) = env.run(HUB_INIT);
    assert_eq!(code, 1);
    assert!(stderr.contains("--force"), "{stderr}");
    assert!(
        env.file("hub.json").contains(CLIENT_PUB),
        "peers must survive"
    );

    let mut args = HUB_INIT.to_vec();
    args.push("--force");
    let (code, stdout, stderr) = env.run(&args);
    assert_eq!(code, 0, "{stderr}");
    assert!(stdout.contains("backed up"), "{stdout}");
    assert!(!env.file("hub.json").contains(CLIENT_PUB));

    let backups: Vec<String> = fs::read_dir(env.root.join("wg"))
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|name| name.contains(".bak-"))
        .collect();
    assert_eq!(backups.len(), 3, "{backups:?}");
    for prefix in ["hub.json.bak-", "hub.key.bak-", "lmice0.conf.bak-"] {
        let name = backups
            .iter()
            .find(|name| name.starts_with(prefix))
            .unwrap();
        let path = env.root.join("wg").join(name);
        let mode = fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600, "{name}");
        if prefix == "hub.json.bak-" {
            assert!(fs::read_to_string(&path).unwrap().contains(CLIENT_PUB));
        }
    }
}

#[test]
fn peer_addresses_and_keys_are_validated() {
    let env = setup(HUB_PRIV, HUB_PUB, HUB_DUMP);
    assert_eq!(env.run(HUB_INIT).0, 0);
    assert_eq!(add_peer(&env, "laptop", CLIENT_PUB, "10.60.0.2").0, 0);

    for (address, expected) in [
        ("999.1.1.1", "not a valid IP address"),
        ("10.61.0.2", "not a host address in the hub subnet"),
        ("10.60.0.255", "not a host address in the hub subnet"),
        ("10.60.0.0", "not a host address in the hub subnet"),
        ("fd00::2", "not a host address in the hub subnet"),
        ("10.60.0.1", "hub's own address"),
    ] {
        let (code, _, stderr) = add_peer(&env, "x", SECOND_PUB, address);
        assert_eq!(code, 2, "{address}: {stderr}");
        assert!(stderr.contains(expected), "{address}: {stderr}");
    }

    let (code, _, stderr) = add_peer(&env, "x", HUB_PUB, "10.60.0.3");
    assert_eq!(code, 2);
    assert!(stderr.contains("hub's own public key"), "{stderr}");

    // 43 chars, no padding: decodes to 32 bytes in some tools, but not wg's form.
    let (code, _, stderr) = add_peer(&env, "x", &SECOND_PUB[..43], "10.60.0.3");
    assert_eq!(code, 2);
    assert!(stderr.contains("not a valid WireGuard"), "{stderr}");

    let (code, _, stderr) = env.run(&[
        "wg",
        "client-init",
        "--address",
        "10.60.0.2/24",
        "--endpoint",
        "hub.example.com:51820",
        "--hub-pubkey",
        HUB_PUB,
        "--allowed-ips",
        "10.60.0.0/24, 999.0.0.0/8",
    ]);
    assert_eq!(code, 2);
    assert!(stderr.contains("999.0.0.0/8"), "{stderr}");

    let (code, _, stderr) = env.run(&[
        "wg",
        "hub-init",
        "--address",
        "999.60.0.1/24",
        "--endpoint",
        "hub.example.com:51820",
        "--force",
    ]);
    assert_eq!(code, 2);
    assert!(stderr.contains("not a valid address/prefix"), "{stderr}");
}
