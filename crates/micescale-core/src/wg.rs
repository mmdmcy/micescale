use serde::{Deserialize, Serialize};

pub const DEFAULT_INTERFACE: &str = "lmice0";
pub const DEFAULT_PORT: u16 = 51820;
pub const DEFAULT_PERSISTENT_KEEPALIVE: u16 = 25;

/// True for base64-encoded 32-byte WireGuard keys (43 or 44 chars).
pub fn is_wg_key(value: &str) -> bool {
    (value.len() == 43 || value.len() == 44)
        && value.bytes().all(|byte| {
            byte.is_ascii_alphanumeric() || byte == b'+' || byte == b'/' || byte == b'='
        })
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
