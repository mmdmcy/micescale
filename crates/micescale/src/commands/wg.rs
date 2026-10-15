use std::env;
use std::fs;
use std::net::IpAddr;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use micescale_core::config as mconfig;
use micescale_core::wg::{self, Cidr, ClientState, HubPeer, HubState, WgDump};

use crate::AppError;
use crate::commands::enroll::append_event;
use crate::files::write_private;
use crate::process;

pub const WG_BIN_ENV: &str = "MICESCALE_WG_BIN";
pub const WGQUICK_BIN_ENV: &str = "MICESCALE_WGQUICK_BIN";
pub const WG_DIR_ENV: &str = "MICESCALE_WG_DIR";

fn wg_bin() -> String {
    env::var_os(WG_BIN_ENV)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_string_lossy().into_owned())
        .unwrap_or_else(|| "wg".into())
}

fn wgquick_bin() -> String {
    env::var_os(WGQUICK_BIN_ENV)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_string_lossy().into_owned())
        .unwrap_or_else(|| "wg-quick".into())
}

fn wg_dir() -> std::path::PathBuf {
    env::var_os(WG_DIR_ENV)
        .filter(|value| !value.is_empty())
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| micescale_core::paths::state_dir().join("wireguard"))
}

fn state_path(file: &str) -> std::path::PathBuf {
    wg_dir().join(file)
}

/// The wg-quick config for `interface`. wg-quick names the interface after
/// the file stem, so the file must be `<interface>.conf` for status/doctor
/// (which look the interface up by name) to agree with what `up` creates.
fn conf_path(interface: &str) -> PathBuf {
    state_path(&format!("{interface}.conf"))
}

fn genkey() -> Result<String, AppError> {
    let output = process::run(&wg_bin(), &["genkey"])?;
    output.ok("wg genkey")?;
    let key = output.stdout.trim().to_string();
    if !wg::is_wg_key(&key) {
        return Err(AppError::Operational(format!(
            "wg genkey returned an unexpected key format from {}",
            wg_bin()
        )));
    }
    Ok(key)
}

fn pubkey(private: &str) -> Result<String, AppError> {
    let output = process::run_with_input(&wg_bin(), &["pubkey"], private)?;
    output.ok("wg pubkey")?;
    let key = output.stdout.trim().to_string();
    if !wg::is_wg_key(&key) {
        return Err(AppError::Operational(format!(
            "wg pubkey returned an unexpected key format from {}",
            wg_bin()
        )));
    }
    Ok(key)
}

fn validate_endpoint(endpoint: &str) -> Result<(), AppError> {
    let Some((host, port)) = endpoint.rsplit_once(':') else {
        return Err(AppError::Usage(
            "endpoint must be host:port, for example hub.example.com:51820".into(),
        ));
    };
    if host.is_empty() {
        return Err(AppError::Usage("endpoint host must not be empty".into()));
    }
    let port: u16 = port
        .parse()
        .map_err(|_| AppError::Usage("endpoint port must be a number".into()))?;
    if port == 0 {
        return Err(AppError::Usage("endpoint port must not be 0".into()));
    }
    Ok(())
}

fn validate_interface(interface: &str) -> Result<(), AppError> {
    if !wg::is_interface_name(interface) {
        return Err(AppError::Usage(
            "interface must be 1-15 characters of a-z, A-Z, 0-9, _=+.-".into(),
        ));
    }
    Ok(())
}

/// Parse an address with prefix length (hub/client tunnel address).
fn parse_cidr(address: &str) -> Result<Cidr, AppError> {
    if !address.contains('/') {
        return Err(AppError::Usage(
            "address must include a prefix length, for example 10.60.0.1/24".into(),
        ));
    }
    let cidr = Cidr::parse(address)
        .ok_or_else(|| AppError::Usage(format!("{address} is not a valid address/prefix")))?;
    if cidr.is_reserved(cidr.addr) {
        return Err(AppError::Usage(format!(
            "{address} is a network or broadcast address, not a host address"
        )));
    }
    Ok(cidr)
}

/// Parse a peer address: a plain host IP (subnet checks need the hub state).
fn parse_peer_ip(address: &str) -> Result<IpAddr, AppError> {
    if address.contains('/') {
        return Err(AppError::Usage(
            "peer address must be a plain IP, for example 10.60.0.2".into(),
        ));
    }
    address
        .parse()
        .map_err(|_| AppError::Usage(format!("{address} is not a valid IP address")))
}

/// A peer must be a host address inside the hub subnet, other than the hub's.
fn check_peer_in_subnet(ip: IpAddr, hub: &HubState) -> Result<(), AppError> {
    let subnet = Cidr::parse(&hub.address).ok_or_else(|| {
        AppError::Operational(format!("hub state has an invalid address {}", hub.address))
    })?;
    if !subnet.contains(ip) || subnet.is_reserved(ip) {
        return Err(AppError::Usage(format!(
            "peer address {ip} is not a host address in the hub subnet {}",
            hub.address
        )));
    }
    if ip == subnet.addr {
        return Err(AppError::Usage(format!(
            "peer address {ip} is the hub's own address"
        )));
    }
    Ok(())
}

fn load_hub_state() -> Result<HubState, AppError> {
    let path = state_path("hub.json");
    let source = fs::read_to_string(&path).map_err(|_| {
        AppError::Operational(format!(
            "no hub state at {}; run `micescale wg hub-init` first",
            path.display()
        ))
    })?;
    serde_json::from_str(&source).map_err(|error| {
        AppError::Operational(format!(
            "cannot parse hub state {}: {error}",
            path.display()
        ))
    })
}

fn load_client_state() -> Result<ClientState, AppError> {
    let path = state_path("client.json");
    let source = fs::read_to_string(&path).map_err(|_| {
        AppError::Operational(format!(
            "no client state at {}; run `micescale wg client-init` first",
            path.display()
        ))
    })?;
    serde_json::from_str(&source).map_err(|error| {
        AppError::Operational(format!(
            "cannot parse client state {}: {error}",
            path.display()
        ))
    })
}

fn write_hub_state(state: &HubState) -> Result<(), AppError> {
    fs::write(
        state_path("hub.json"),
        serde_json::to_string_pretty(state).expect("serializes"),
    )
    .map_err(|error| {
        AppError::Operational(format!(
            "cannot write {}: {error}",
            state_path("hub.json").display()
        ))
    })
}

fn render_hub_config() -> Result<(), AppError> {
    let state = load_hub_state()?;
    let key_path = state_path("hub.key");
    let private = fs::read_to_string(&key_path).map_err(|_| {
        AppError::Operational(format!(
            "hub private key missing at {}; re-run `micescale wg hub-init`",
            key_path.display()
        ))
    })?;
    let private = private.trim();
    if !wg::is_wg_key(private) {
        return Err(AppError::Operational(
            "hub.key does not contain a valid WireGuard key".into(),
        ));
    }
    write_private(&conf_path(&state.interface), &state.render_config(private))
}

/// Copy existing hub files to `<file>.bak-<unix-seconds>` (0600) so a forced
/// re-init never loses the old key or peer registry.
fn backup_hub_state() -> Result<Vec<PathBuf>, AppError> {
    let mut files = vec![state_path("hub.json"), state_path("hub.key")];
    if let Ok(state) = load_hub_state() {
        files.push(conf_path(&state.interface));
    }
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock after epoch")
        .as_secs();
    let mut written = Vec::new();
    for path in files.into_iter().filter(|path| path.exists()) {
        let backup = PathBuf::from(format!("{}.bak-{ts}", path.display()));
        if backup.exists() {
            return Err(AppError::Operational(format!(
                "backup {} already exists; retry in a second",
                backup.display()
            )));
        }
        let content = fs::read_to_string(&path).map_err(|error| {
            AppError::Operational(format!("cannot read {}: {error}", path.display()))
        })?;
        write_private(&backup, &content)?;
        written.push(backup);
    }
    Ok(written)
}

pub fn hub_init(
    listen_port: u16,
    address: &str,
    endpoint: &str,
    interface: &str,
    force: bool,
) -> Result<(), AppError> {
    validate_endpoint(endpoint)?;
    parse_cidr(address)?;
    validate_interface(interface)?;
    if listen_port == 0 {
        return Err(AppError::Usage("listen-port must not be 0".into()));
    }
    let existing = state_path("hub.json").exists() || state_path("hub.key").exists();
    if existing && !force {
        return Err(AppError::Operational(format!(
            "hub state already exists in {}; re-running hub-init would replace the hub key and drop every peer. Pass --force to back it up and start over",
            wg_dir().display()
        )));
    }
    fs::create_dir_all(wg_dir()).map_err(|error| {
        AppError::Operational(format!("cannot create {}: {error}", wg_dir().display()))
    })?;
    if existing {
        for backup in backup_hub_state()? {
            println!("backed up {}", backup.display());
        }
    }
    let private = genkey()?;
    let public = pubkey(&private)?;
    let state = HubState {
        interface: interface.to_string(),
        listen_port,
        address: address.to_string(),
        endpoint: endpoint.to_string(),
        pubkey: public.clone(),
        peers: Vec::new(),
    };
    write_hub_state(&state)?;
    write_private(&state_path("hub.key"), &format!("{private}\n"))?;
    write_private(&conf_path(interface), &state.render_config(&private))?;

    let config = mconfig::Config::new("wireguard".into(), None, None);
    mconfig::save(&mconfig::default_path(), &config)?;
    let detail = if existing {
        "hub keypair regenerated (--force); old state backed up"
    } else {
        "hub keypair generated"
    };
    append_event(&config, "wg-hub-init", "ok", Some(detail))?;

    println!("hub initialized");
    println!("  public key : {public}");
    println!("  endpoint   : {endpoint}");
    println!("  interface  : {}", state.interface);
    println!("  subnet     : {}", state.address);
    println!("state written to {}", wg_dir().display());
    println!("share the public key with clients; they must run `micescale wg client-init`.");
    Ok(())
}

pub fn hub_add_peer(name: &str, pubkey: &str, address: &str) -> Result<(), AppError> {
    if name.trim().is_empty() {
        return Err(AppError::Usage("peer name must not be empty".into()));
    }
    if !wg::is_wg_key(pubkey) {
        return Err(AppError::Usage(
            "pubkey is not a valid WireGuard public key".into(),
        ));
    }
    let ip = parse_peer_ip(address)?;
    let mut state = load_hub_state()?;
    if pubkey == state.pubkey {
        return Err(AppError::Usage(
            "pubkey is the hub's own public key; pass the client's key".into(),
        ));
    }
    check_peer_in_subnet(ip, &state)?;
    let address = ip.to_string();
    if let Some(existing) = state.find_peer(name) {
        return Err(AppError::Operational(format!(
            "peer {name} already exists at {}",
            existing.address
        )));
    }
    if state
        .peers
        .iter()
        .any(|peer| peer.pubkey == pubkey || peer.address.parse::<IpAddr>().ok() == Some(ip))
    {
        return Err(AppError::Operational(
            "a peer with this public key or address already exists".into(),
        ));
    }
    state.peers.push(HubPeer {
        name: name.to_string(),
        pubkey: pubkey.to_string(),
        address: address.clone(),
    });
    write_hub_state(&state)?;
    render_hub_config()?;

    let config = mconfig::Config::new("wireguard".into(), None, None);
    append_event(&config, "wg-hub-add-peer", "ok", Some(name))?;

    println!("peer {name} added at {address}");
    print_apply_hint(&state.interface);
    Ok(())
}

pub fn hub_remove_peer(name: &str) -> Result<(), AppError> {
    let mut state = load_hub_state()?;
    let before = state.peers.len();
    state.peers.retain(|peer| peer.name != name);
    if state.peers.len() == before {
        return Err(AppError::Operational(format!("no peer named {name}")));
    }
    write_hub_state(&state)?;
    render_hub_config()?;
    let config = mconfig::Config::new("wireguard".into(), None, None);
    append_event(&config, "wg-hub-remove-peer", "ok", Some(name))?;
    println!("peer {name} removed");
    print_apply_hint(&state.interface);
    Ok(())
}

pub fn hub_render() -> Result<(), AppError> {
    let state = load_hub_state()?;
    render_hub_config()?;
    println!(
        "hub config re-rendered at {}",
        conf_path(&state.interface).display()
    );
    print_apply_hint(&state.interface);
    Ok(())
}

/// The commands that load a re-rendered hub config. Both wg-quick and wg need
/// root, and wg-quick only finds bare interface names in /etc/wireguard, so
/// strip is given the full path and the whole pipeline runs under one sudo.
fn apply_hint(interface: &str) -> String {
    let path = conf_path(interface);
    format!(
        "apply the change: sudo micescale wg up (if down), or live: sudo bash -c 'wg syncconf {interface} <(wg-quick strip {})'",
        path.display()
    )
}

fn print_apply_hint(interface: &str) {
    println!("{}", apply_hint(interface));
}

pub fn client_init(
    address: &str,
    endpoint: &str,
    hub_pubkey: &str,
    allowed_ips: &str,
    interface: &str,
) -> Result<(), AppError> {
    parse_cidr(address)?;
    validate_endpoint(endpoint)?;
    validate_interface(interface)?;
    if !wg::is_wg_key(hub_pubkey) {
        return Err(AppError::Usage(
            "hub-pubkey is not a valid WireGuard public key".into(),
        ));
    }
    if allowed_ips.trim().is_empty() {
        return Err(AppError::Usage("allowed-ips must not be empty".into()));
    }
    for prefix in allowed_ips.split(',').map(str::trim) {
        if Cidr::parse(prefix).is_none() {
            return Err(AppError::Usage(format!(
                "allowed-ips entry {prefix} is not a valid address/prefix"
            )));
        }
    }
    fs::create_dir_all(wg_dir()).map_err(|error| {
        AppError::Operational(format!("cannot create {}: {error}", wg_dir().display()))
    })?;
    let private = genkey()?;
    let public = pubkey(&private)?;
    let state = ClientState {
        interface: interface.to_string(),
        address: address.to_string(),
        endpoint: endpoint.to_string(),
        hub_pubkey: hub_pubkey.to_string(),
        allowed_ips: allowed_ips.to_string(),
        pubkey: public.clone(),
    };
    fs::write(
        state_path("client.json"),
        serde_json::to_string_pretty(&state).expect("serializes"),
    )
    .map_err(|error| {
        AppError::Operational(format!(
            "cannot write {}: {error}",
            state_path("client.json").display()
        ))
    })?;
    write_private(&conf_path(interface), &state.render_config(&private))?;

    let config = mconfig::Config::new("wireguard".into(), None, None);
    mconfig::save(&mconfig::default_path(), &config)?;
    append_event(&config, "wg-enroll", "ok", Some("client keypair generated"))?;

    println!("client initialized");
    println!("  public key : {public}");
    println!("  address    : {}", state.address);
    println!("  hub        : {}", state.endpoint);
    println!("state written to {}", wg_dir().display());
    let address_without_mask = address.split('/').next().unwrap_or(address).to_string();
    println!(
        "on the hub, run: micescale wg hub-add-peer --name <this-host> --pubkey {public} --address {address_without_mask}"
    );
    println!("then bring the link up: sudo micescale wg up");
    Ok(())
}

fn active_config_path() -> Result<Option<(String, std::path::PathBuf)>, AppError> {
    if state_path("client.json").exists() {
        let state = load_client_state()?;
        let path = conf_path(&state.interface);
        return Ok(Some((state.interface, path)));
    }
    if state_path("hub.json").exists() {
        let state = load_hub_state()?;
        let path = conf_path(&state.interface);
        return Ok(Some((state.interface, path)));
    }
    Ok(None)
}

pub fn up() -> Result<(), AppError> {
    let Some((interface, path)) = active_config_path()? else {
        return Err(AppError::Operational(
            "no wireguard state; run `micescale wg hub-init` or `micescale wg client-init` first"
                .into(),
        ));
    };
    process::run(
        &wgquick_bin(),
        &["up", path.to_str().expect("path is utf8")],
    )?
    .ok("wg-quick up")?;
    let config = mconfig::Config::new("wireguard".into(), None, None);
    append_event(&config, "wg-up", "ok", Some(&interface))?;
    println!("{interface} is up");
    Ok(())
}

pub fn down() -> Result<(), AppError> {
    let Some((interface, path)) = active_config_path()? else {
        return Err(AppError::Operational(
            "no wireguard state; run `micescale wg hub-init` or `micescale wg client-init` first"
                .into(),
        ));
    };
    process::run(
        &wgquick_bin(),
        &["down", path.to_str().expect("path is utf8")],
    )?
    .ok("wg-quick down")?;
    let config = mconfig::Config::new("wireguard".into(), None, None);
    append_event(&config, "wg-down", "ok", Some(&interface))?;
    println!("{interface} is down");
    Ok(())
}

pub fn leave() -> Result<(), AppError> {
    let client_conf = load_client_state()
        .ok()
        .map(|state| conf_path(&state.interface));
    down()?;
    for path in [Some(state_path("client.json")), client_conf]
        .into_iter()
        .flatten()
    {
        if path.exists() {
            fs::remove_file(&path).map_err(|error| {
                AppError::Operational(format!("cannot remove {}: {error}", path.display()))
            })?;
        }
    }
    let config = mconfig::Config::new("wireguard".into(), None, None);
    append_event(&config, "wg-leave", "ok", Some("client state removed"))?;
    println!("left the wireguard mesh");
    Ok(())
}

pub fn status(format: &str) -> Result<(), AppError> {
    let report = if state_path("client.json").exists() {
        let state = load_client_state()?;
        let dump = wg_dump()?;
        build_client_status(&state, &dump)
    } else if state_path("hub.json").exists() {
        let state = load_hub_state()?;
        let dump = wg_dump()?;
        build_status(&state, &dump)
    } else {
        return Err(AppError::Operational(
            "no wireguard state; run `micescale wg hub-init` or `micescale wg client-init` first"
                .into(),
        ));
    };
    match format {
        "json" => println!(
            "{}",
            serde_json::to_string_pretty(&report).expect("serializes")
        ),
        "yaml" => println!("{}", serde_yaml_ng::to_string(&report).expect("serializes")),
        _ => print_text(&report),
    }
    Ok(())
}

#[derive(serde::Serialize)]
pub struct WgStatusReport {
    pub product: &'static str,
    pub posture: &'static str,
    pub carrier: &'static str,
    pub side: &'static str,
    pub interface: String,
    pub running: bool,
    pub address: String,
    pub endpoint: String,
    pub pubkey: String,
    pub peers: Vec<PeerStatus>,
}

#[derive(serde::Serialize)]
pub struct PeerStatus {
    pub name: String,
    pub pubkey: String,
    pub endpoint: Option<String>,
    pub allowed_ips: String,
    pub latest_handshake: Option<u64>,
    pub transfer_rx: u64,
    pub transfer_tx: u64,
    pub online: bool,
}

fn wg_dump() -> Result<WgDump, AppError> {
    let output = process::run(&wg_bin(), &["show", "all", "dump"])?;
    if output.status != Some(0) {
        return Err(AppError::Operational(format!(
            "wg show all dump failed: {}",
            output.stderr.trim()
        )));
    }
    wg::parse_dump(&output.stdout)
        .ok_or_else(|| AppError::Operational("cannot parse `wg show all dump` output".into()))
}

fn build_status(state: &HubState, dump: &WgDump) -> WgStatusReport {
    let running = dump
        .interfaces
        .iter()
        .any(|interface| interface.interface == state.interface);
    let peers = state
        .peers
        .iter()
        .map(|peer| {
            let line = dump
                .peers
                .iter()
                .find(|line| line.interface == state.interface && line.pubkey == peer.pubkey);
            PeerStatus {
                name: peer.name.clone(),
                pubkey: peer.pubkey.clone(),
                endpoint: line.and_then(|line| line.endpoint.clone()),
                allowed_ips: line
                    .map(|line| line.allowed_ips.clone())
                    .unwrap_or_else(|| format!("{}/32", peer.address)),
                latest_handshake: line.map(|line| line.latest_handshake),
                transfer_rx: line.map(|line| line.transfer_rx).unwrap_or(0),
                transfer_tx: line.map(|line| line.transfer_tx).unwrap_or(0),
                online: line.is_some_and(|line| line.latest_handshake > 0),
            }
        })
        .collect();
    WgStatusReport {
        product: micescale_core::PRODUCT,
        posture: micescale_core::carrier::POSTURE,
        carrier: "wireguard",
        side: "hub",
        interface: state.interface.clone(),
        running,
        address: state.address.clone(),
        endpoint: state.endpoint.clone(),
        pubkey: state.pubkey.clone(),
        peers,
    }
}

fn build_client_status(state: &ClientState, dump: &WgDump) -> WgStatusReport {
    let running = dump
        .interfaces
        .iter()
        .any(|interface| interface.interface == state.interface);
    let line = dump
        .peers
        .iter()
        .find(|line| line.interface == state.interface && line.pubkey == state.hub_pubkey);
    WgStatusReport {
        product: micescale_core::PRODUCT,
        posture: micescale_core::carrier::POSTURE,
        carrier: "wireguard",
        side: "client",
        interface: state.interface.clone(),
        running,
        address: state.address.clone(),
        endpoint: state.endpoint.clone(),
        pubkey: state.pubkey.clone(),
        peers: vec![PeerStatus {
            name: "hub".into(),
            pubkey: state.hub_pubkey.clone(),
            endpoint: line.and_then(|line| line.endpoint.clone()),
            allowed_ips: state.allowed_ips.clone(),
            latest_handshake: line.map(|line| line.latest_handshake),
            transfer_rx: line.map(|line| line.transfer_rx).unwrap_or(0),
            transfer_tx: line.map(|line| line.transfer_tx).unwrap_or(0),
            online: line.is_some_and(|line| line.latest_handshake > 0),
        }],
    }
}

fn print_text(report: &WgStatusReport) {
    println!("posture: {}", report.posture);
    println!(
        "carrier: {} ({})",
        report.carrier,
        if report.running { "running" } else { "down" }
    );
    println!("side: {}", report.side);
    println!("interface: {}", report.interface);
    println!("address: {}", report.address);
    println!("endpoint: {}", report.endpoint);
    println!("public key: {}", report.pubkey);
    for peer in &report.peers {
        println!(
            "peer: {:<16} {:<5} handshake={} rx={} tx={}",
            peer.name,
            if peer.online { "online" } else { "offline" },
            peer.latest_handshake
                .map(|ts| {
                    let now = SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .expect("clock")
                        .as_secs();
                    format!("{}s ago", now.saturating_sub(ts))
                })
                .unwrap_or_else(|| "never".into()),
            peer.transfer_rx,
            peer.transfer_tx,
        );
    }
}

/// Dispatch used by the top-level `micescale doctor` command.
pub fn top_level_doctor() -> Result<micescale_core::health::HealthReport, AppError> {
    let mut report = micescale_core::health::HealthReport::new("wireguard".into());
    let wg_binary = wg_bin();
    let wg_present = process::run(&wg_binary, &["version"]).is_ok();
    report.add(micescale_core::health::HealthCheck {
        name: "wg-binary".into(),
        ok: wg_present,
        detail: Some(if wg_present {
            format!("{wg_binary} present")
        } else {
            format!("{wg_binary} not found; install wireguard-tools")
        }),
    });
    let active_interface = if state_path("client.json").exists() {
        load_client_state().ok().map(|state| state.interface)
    } else if state_path("hub.json").exists() {
        load_hub_state().ok().map(|state| state.interface)
    } else {
        None
    };
    let state_ok = active_interface.is_some();
    report.add(micescale_core::health::HealthCheck {
        name: "wg-state".into(),
        ok: state_ok,
        detail: Some(if state_ok {
            "wireguard state present".into()
        } else {
            "run `micescale wg hub-init` or `micescale wg client-init` first".into()
        }),
    });
    let running = active_interface
        .and_then(|interface| {
            wg_dump().ok().map(|dump| {
                dump.interfaces
                    .iter()
                    .any(|line| line.interface == interface)
            })
        })
        .unwrap_or(false);
    report.add(micescale_core::health::HealthCheck {
        name: "interface".into(),
        ok: running,
        detail: Some(if running {
            "interface up".into()
        } else {
            "interface down; run `sudo micescale wg up`".into()
        }),
    });
    Ok(report)
}
