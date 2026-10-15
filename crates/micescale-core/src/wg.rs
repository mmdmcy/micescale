use std::net::IpAddr;

use serde::{Deserialize, Serialize};

pub const DEFAULT_INTERFACE: &str = "lmice0";
pub const DEFAULT_PORT: u16 = 51820;
pub const DEFAULT_PERSISTENT_KEEPALIVE: u16 = 25;

const BASE64: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// True for canonical base64 encodings of a 32-byte WireGuard key: 43 base64
/// characters plus one `=`, with the unused low bits of the last character
/// zero (the only form `wg` itself emits and accepts).
pub fn is_wg_key(value: &str) -> bool {
    let bytes = value.as_bytes();
    if bytes.len() != 44 || bytes[43] != b'=' {
        return false;
    }
    let mut last = 0;
    for byte in &bytes[..43] {
        match BASE64.iter().position(|candidate| candidate == byte) {
            Some(index) => last = index,
            None => return false,
        }
    }
    last & 0b11 == 0
}

/// True for names `wg-quick` accepts as an interface (it names the interface
/// after the config file stem, so `<name>.conf` must pass this too).
pub fn is_interface_name(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 15
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"_=+.-".contains(&byte))
}

/// An address with prefix length, e.g. 10.60.0.1/24.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cidr {
    pub addr: IpAddr,
    pub prefix: u8,
}

impl Cidr {
    pub fn parse(value: &str) -> Option<Cidr> {
        let (addr, prefix) = value.split_once('/')?;
        let addr: IpAddr = addr.parse().ok()?;
        let prefix: u8 = prefix.parse().ok()?;
        let max = if addr.is_ipv4() { 32 } else { 128 };
        (prefix <= max).then_some(Cidr { addr, prefix })
    }

    pub fn contains(&self, ip: IpAddr) -> bool {
        match (self.addr, ip) {
            (IpAddr::V4(net), IpAddr::V4(ip)) => {
                let mask = u32::MAX
                    .checked_shl(32 - u32::from(self.prefix))
                    .unwrap_or(0);
                u32::from(net) & mask == u32::from(ip) & mask
            }
            (IpAddr::V6(net), IpAddr::V6(ip)) => {
                let mask = u128::MAX
                    .checked_shl(128 - u32::from(self.prefix))
                    .unwrap_or(0);
                u128::from(net) & mask == u128::from(ip) & mask
            }
            _ => false,
        }
    }

    /// True when `ip` has the host bits of an IPv4 network or broadcast
    /// address for this prefix (/31 and /32 have neither).
    pub fn is_reserved(&self, ip: IpAddr) -> bool {
        let IpAddr::V4(ip) = ip else {
            return false;
        };
        if !self.addr.is_ipv4() || self.prefix >= 31 {
            return false;
        }
        let host_mask = u32::MAX >> self.prefix;
        let host = u32::from(ip) & host_mask;
        host == 0 || host == host_mask
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HubPeer {
    pub name: String,
    pub pubkey: String,
    pub address: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HubState {
    pub interface: String,
    pub listen_port: u16,
    pub address: String,
    pub endpoint: String,
    pub pubkey: String,
    pub peers: Vec<HubPeer>,
}

impl HubState {
    pub fn render_config(&self, private_key: &str) -> String {
        let mut out = String::new();
        out.push_str("[Interface]\n");
        out.push_str(&format!("PrivateKey = {private_key}\n"));
        out.push_str(&format!("Address = {}\n", self.address));
        out.push_str(&format!("ListenPort = {}\n", self.listen_port));
        // Spokes reach each other through the hub, so it must forward between
        // peers on its own interface. The sysctl is left on at PostDown.
        let (sysctl, iptables) = match Cidr::parse(&self.address).map(|cidr| cidr.addr) {
            Some(IpAddr::V6(_)) => ("net.ipv6.conf.all.forwarding", "ip6tables"),
            _ => ("net.ipv4.ip_forward", "iptables"),
        };
        out.push_str(&format!(
            "PostUp = sysctl -w {sysctl}=1; {iptables} -I FORWARD -i %i -o %i -j ACCEPT\n"
        ));
        out.push_str(&format!(
            "PostDown = {iptables} -D FORWARD -i %i -o %i -j ACCEPT\n"
        ));
        for peer in &self.peers {
            out.push('\n');
            out.push_str("[Peer]\n");
            out.push_str(&format!("PublicKey = {}\n", peer.pubkey));
            out.push_str(&format!("AllowedIPs = {}/32\n", peer.address));
        }
        out
    }

    pub fn find_peer(&self, name: &str) -> Option<&HubPeer> {
        self.peers.iter().find(|peer| peer.name == name)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ClientState {
    pub interface: String,
    pub address: String,
    pub endpoint: String,
    pub hub_pubkey: String,
    pub allowed_ips: String,
    pub pubkey: String,
}

impl ClientState {
    pub fn render_config(&self, private_key: &str) -> String {
        format!(
            "[Interface]\nPrivateKey = {private_key}\nAddress = {}\n\n[Peer]\nPublicKey = {}\nEndpoint = {}\nAllowedIPs = {}\nPersistentKeepalive = {}\n",
            self.address,
            self.hub_pubkey,
            self.endpoint,
            self.allowed_ips,
            DEFAULT_PERSISTENT_KEEPALIVE
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InterfaceLine {
    pub interface: String,
    pub pubkey: String,
    pub listen_port: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PeerLine {
    pub interface: String,
    pub pubkey: String,
    pub endpoint: Option<String>,
    pub allowed_ips: String,
    pub latest_handshake: u64,
    pub transfer_rx: u64,
    pub transfer_tx: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct WgDump {
    pub interfaces: Vec<InterfaceLine>,
    pub peers: Vec<PeerLine>,
}

/// Parse `wg show all dump` output (tab-separated, one interface line followed
/// by its peer lines). Returns None when the first line is malformed.
pub fn parse_dump(source: &str) -> Option<WgDump> {
    let mut dump = WgDump::default();
    let mut current_interface = String::new();
    for (index, line) in source.lines().enumerate() {
        let fields: Vec<&str> = line.split('\t').collect();
        if index == 0 && fields.len() < 2 {
            return None;
        }
        match fields.len() {
            5 | 6 => {
                let interface = fields[0].to_string();
                dump.interfaces.push(InterfaceLine {
                    listen_port: fields[3].parse().unwrap_or(0),
                    pubkey: fields[1].to_string(),
                    interface: interface.clone(),
                });
                current_interface = interface;
            }
            9 => {
                dump.peers.push(PeerLine {
                    interface: current_interface.clone(),
                    pubkey: fields[1].to_string(),
                    endpoint: {
                        let value = fields[3];
                        if value.is_empty() {
                            None
                        } else {
                            Some(value.to_string())
                        }
                    },
                    allowed_ips: fields[4].to_string(),
                    latest_handshake: fields[5].parse().unwrap_or(0),
                    transfer_rx: fields[6].parse().unwrap_or(0),
                    transfer_tx: fields[7].parse().unwrap_or(0),
                });
            }
            _ => return None,
        }
    }
    Some(dump)
}

#[cfg(test)]
mod tests {
    use super::*;

    const HUB_PRIVATE: &str = "aHViLXByaXZhdGUta2V5LWNvcmUtaHViLXByaXZhdGU=";
    const HUB_PUBLIC: &str = "aHViLXB1YmxpYy1rZXktY29yZS1odWItcHVibGljLWs=";
    const CLIENT_PUBLIC: &str = "Y2xpZW50LXB1YmxpYy1rZXktY29yZS1jbGllbnQtcHU=";

    #[test]
    fn key_validation_accepts_standard_keys() {
        assert!(is_wg_key(HUB_PUBLIC));
        assert!(is_wg_key(CLIENT_PUBLIC));
        assert!(!is_wg_key("short"));
        assert!(!is_wg_key("not base64 !!! !!! !!! !!! !!! !!! !!! !!! 12"));
        assert!(!is_wg_key("eHh4eHh4eHh4eHh4eHh4eHh4eHh4eHh4eHh4eHh4eA"));
        assert!(!is_wg_key("eHh4eHh4eHh4eHh4eHh4eHh4eHh4eHh4eHh4eHh4eHg==x"));
    }

    #[test]
    fn key_validation_requires_canonical_32_bytes() {
        // 43 chars without padding, wrong padding, non-zero trailing bits.
        assert!(!is_wg_key(&HUB_PUBLIC[..43]));
        assert!(!is_wg_key(&format!("{}A", &HUB_PUBLIC[..43])));
        assert!(!is_wg_key("aHViLXB1YmxpYy1rZXktY29yZS1odWItcHVibGljLWt="));
        assert!(is_wg_key("AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA="));
    }

    #[test]
    fn interface_names_match_wg_quick_rules() {
        assert!(is_interface_name("lmice0"));
        assert!(is_interface_name("wg_hub.1"));
        assert!(!is_interface_name(""));
        assert!(!is_interface_name("sixteen-chars-xx"));
        assert!(!is_interface_name("../etc"));
        assert!(!is_interface_name("a b"));
    }

    #[test]
    fn cidr_parses_and_checks_membership() {
        let net = Cidr::parse("10.60.0.1/24").unwrap();
        assert!(net.contains("10.60.0.2".parse().unwrap()));
        assert!(!net.contains("10.61.0.2".parse().unwrap()));
        assert!(!net.contains("fd00::2".parse().unwrap()));
        assert!(net.is_reserved("10.60.0.0".parse().unwrap()));
        assert!(net.is_reserved("10.60.0.255".parse().unwrap()));
        assert!(!net.is_reserved("10.60.0.254".parse().unwrap()));
        assert!(
            !Cidr::parse("10.60.0.1/31")
                .unwrap()
                .is_reserved("10.60.0.0".parse().unwrap())
        );
        assert!(
            Cidr::parse("0.0.0.0/0")
                .unwrap()
                .contains("8.8.8.8".parse().unwrap())
        );
        let v6 = Cidr::parse("fd00:60::1/64").unwrap();
        assert!(v6.contains("fd00:60::9".parse().unwrap()));
        assert!(!v6.contains("fd00:61::9".parse().unwrap()));
        for bad in [
            "999.1.1.1/24",
            "10.60.0.1",
            "10.60.0.1/33",
            "10.60.0.1/x",
            "fd00::1/129",
        ] {
            assert_eq!(Cidr::parse(bad), None, "{bad}");
        }
    }

    #[test]
    fn hub_render_forwards_between_peers() {
        let mut hub = HubState {
            interface: DEFAULT_INTERFACE.into(),
            listen_port: 51820,
            address: "10.60.0.1/24".into(),
            endpoint: "hub.example.com:51820".into(),
            pubkey: HUB_PUBLIC.into(),
            peers: Vec::new(),
        };
        let rendered = hub.render_config(HUB_PRIVATE);
        assert!(rendered.contains(
            "PostUp = sysctl -w net.ipv4.ip_forward=1; iptables -I FORWARD -i %i -o %i -j ACCEPT\n"
        ));
        assert!(rendered.contains("PostDown = iptables -D FORWARD -i %i -o %i -j ACCEPT\n"));
        hub.address = "fd00:60::1/64".into();
        let rendered = hub.render_config(HUB_PRIVATE);
        assert!(rendered.contains("sysctl -w net.ipv6.conf.all.forwarding=1; ip6tables -I"));
        assert!(rendered.contains("PostDown = ip6tables -D FORWARD"));
    }

    #[test]
    fn parses_multiple_interfaces_and_empty_endpoints() {
        let source = concat!(
            "lmice0\tAHU9KEY\tK3k=\t51820\toff\n",
            "lmice0\tPEERKEY\t\t\t10.60.0.2/32\t0\t0\t0\t25\n",
            "wg0\tOTHERKEY\tYWJj\t12345\toff\n",
        );
        let dump = parse_dump(source).expect("dump parses");
        assert_eq!(dump.interfaces.len(), 2);
        assert_eq!(dump.interfaces[1].interface, "wg0");
        assert_eq!(dump.interfaces[1].listen_port, 12345);
        assert_eq!(dump.peers.len(), 1);
        assert_eq!(dump.peers[0].endpoint, None);
        assert_eq!(dump.peers[0].latest_handshake, 0);
    }

    #[test]
    fn rejects_malformed_dump() {
        assert_eq!(parse_dump("just-an-interface-name"), None);
        assert_eq!(
            parse_dump("lmice0\tK3k=\t51820\toff\nlmice0\tSHORT\t\t1.2.3.4/32\t0\t0\n"),
            None
        );
        assert_eq!(parse_dump(""), Some(WgDump::default()));
    }

    #[test]
    fn find_peer_matches_by_name() {
        let hub = HubState {
            interface: DEFAULT_INTERFACE.into(),
            listen_port: 51820,
            address: "10.60.0.1/24".into(),
            endpoint: "hub.example.com:51820".into(),
            pubkey: HUB_PUBLIC.into(),
            peers: vec![
                HubPeer {
                    name: "laptop".into(),
                    pubkey: CLIENT_PUBLIC.into(),
                    address: "10.60.0.2".into(),
                },
                HubPeer {
                    name: "server".into(),
                    pubkey: "c2Vjb25kLXB1YmxpYy1rZXktc2Vjb25kLXB1YmxpYy0=".into(),
                    address: "10.60.0.3".into(),
                },
            ],
        };
        assert_eq!(
            hub.find_peer("laptop").map(|peer| peer.address.as_str()),
            Some("10.60.0.2")
        );
        assert_eq!(hub.find_peer("ghost"), None);
    }

    #[test]
    fn hub_render_includes_peers() {
        let hub = HubState {
            interface: DEFAULT_INTERFACE.into(),
            listen_port: 51820,
            address: "10.60.0.1/24".into(),
            endpoint: "hub.example.com:51820".into(),
            pubkey: HUB_PUBLIC.into(),
            peers: vec![HubPeer {
                name: "laptop".into(),
                pubkey: CLIENT_PUBLIC.into(),
                address: "10.60.0.2".into(),
            }],
        };
        let rendered = hub.render_config(HUB_PRIVATE);
        assert!(rendered.contains("PrivateKey = aHViLXByaXZhdGUta2V5LWNvcmUtaHViLXByaXZhdGU="));
        assert!(rendered.contains("ListenPort = 51820"));
        assert!(rendered.contains("PublicKey = Y2xpZW50LXB1YmxpYy1rZXktY29yZS1jbGllbnQtcHU="));
        assert!(rendered.contains("AllowedIPs = 10.60.0.2/32"));
    }

    #[test]
    fn client_render_keeps_hub_endpoint() {
        let client = ClientState {
            interface: DEFAULT_INTERFACE.into(),
            address: "10.60.0.2/24".into(),
            endpoint: "hub.example.com:51820".into(),
            hub_pubkey: HUB_PUBLIC.into(),
            allowed_ips: "10.60.0.0/24".into(),
            pubkey: CLIENT_PUBLIC.into(),
        };
        let rendered = client.render_config("Y2xpZW50LXByaXZhdGUta2V5LWNvcmUtY2xpZW50LXByaXZhdGU=");
        assert!(rendered.contains("Endpoint = hub.example.com:51820"));
        assert!(rendered.contains("AllowedIPs = 10.60.0.0/24"));
        assert!(rendered.contains("PersistentKeepalive = 25"));
    }

    #[test]
    fn parses_wg_dump() {
        let source = concat!(
            "lmice0\tAHuBpUbKeYhUbPuBlIcKeYhUbPuBlIcKeYhUbPuBlIcKeY=\tK3k=\t51820\toff\n",
            "lmice0\tCLIENTPUBKEY\t\t10.60.0.2:4\t10.60.0.2/32\t1780000000\t1234\t5678\t25\n",
        );
        let dump = parse_dump(source).expect("dump parses");
        assert_eq!(dump.interfaces.len(), 1);
        assert_eq!(dump.interfaces[0].interface, "lmice0");
        assert_eq!(dump.interfaces[0].listen_port, 51820);
        assert_eq!(dump.peers.len(), 1);
        assert_eq!(dump.peers[0].allowed_ips, "10.60.0.2/32");
        assert_eq!(dump.peers[0].latest_handshake, 1_780_000_000);
        assert_eq!(dump.peers[0].transfer_rx, 1234);
        assert_eq!(dump.peers[0].transfer_tx, 5678);
    }
}
