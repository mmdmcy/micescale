use serde::Serialize;

pub const POSTURE: &str = "carrier-untrusted";
pub const AUTHORIZATION_RULE: &str = "network reachability is never LinuxMice authorization";
pub const ENCRYPTION_RULE: &str =
    "service traffic uses LinuxMice-owned mTLS or encrypted envelopes above any carrier";
pub const METADATA_WARNING: &str =
    "carrier networks may still observe timing, endpoints, packet sizes, and availability signals";

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct CarrierProfile {
    pub id: &'static str,
    pub label: &'static str,
    pub carrier: &'static str,
    pub status: &'static str,
    pub use_case: &'static str,
    pub authorization_boundary: &'static str,
    pub encryption_boundary: &'static str,
    pub enterprise_default: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct CarrierPolicy {
    pub product: &'static str,
    pub posture: &'static str,
    pub authorization_rule: &'static str,
    pub encryption_rule: &'static str,
    pub metadata_warning: &'static str,
    pub profiles: Vec<CarrierProfile>,
    pub enterprise_requirements: Vec<&'static str>,
}

pub fn carrier_profiles() -> Vec<CarrierProfile> {
    vec![
        CarrierProfile {
            id: "localhost",
            label: "Localhost",
            carrier: "loopback",
            status: "supported-mvp",
            use_case: "same-host daemon and CLI workflows",
            authorization_boundary: "LinuxMice certificate, role, and service policy",
            encryption_boundary: "LinuxMice mTLS or encrypted envelope, depending on workflow",
            enterprise_default: true,
        },
        CarrierProfile {
            id: "lan",
            label: "LAN",
            carrier: "customer-controlled local network",
            status: "supported",
            use_case: "site-local endpoints and control-plane access",
            authorization_boundary: "LinuxMice certificate, role, and service policy",
            encryption_boundary: "LinuxMice mTLS plus optional payload envelopes",
            enterprise_default: true,
        },
        CarrierProfile {
            id: "wireguard",
            label: "WireGuard",
            carrier: "self-managed WireGuard",
            status: "supported",
            use_case: "customer-managed routed overlay",
            authorization_boundary: "LinuxMice certificate, role, and service policy",
            encryption_boundary: "WireGuard carrier encryption plus LinuxMice service encryption",
            enterprise_default: true,
        },
        CarrierProfile {
            id: "headscale",
            label: "Headscale",
            carrier: "self-hosted coordination for a WireGuard mesh",
            status: "supported",
            use_case: "Tailscale-like UX with customer-owned coordination",
            authorization_boundary: "LinuxMice certificate, role, and service policy",
            encryption_boundary: "WireGuard carrier encryption plus LinuxMice service encryption",
            enterprise_default: true,
        },
        CarrierProfile {
            id: "tailscale",
            label: "Tailscale",
            carrier: "third-party coordination for a WireGuard mesh",
            status: "supported-carrier",
            use_case: "dogfood connectivity and customer deployments that choose it",
            authorization_boundary: "LinuxMice certificate, role, and service policy",
            encryption_boundary: "Tailscale carrier encryption plus LinuxMice service encryption",
            enterprise_default: false,
        },
        CarrierProfile {
            id: "ssh-tunnel",
            label: "SSH tunnel",
            carrier: "operator-managed SSH transport",
            status: "planned",
            use_case: "break-glass or migration access",
            authorization_boundary: "LinuxMice certificate, role, and service policy",
            encryption_boundary: "SSH carrier encryption plus LinuxMice service encryption",
            enterprise_default: false,
        },
    ]
}

pub fn carrier_policy() -> CarrierPolicy {
    CarrierPolicy {
        product: crate::PRODUCT,
        posture: POSTURE,
        authorization_rule: AUTHORIZATION_RULE,
        encryption_rule: ENCRYPTION_RULE,
        metadata_warning: METADATA_WARNING,
        profiles: carrier_profiles(),
        enterprise_requirements: vec![
            "customer-controlled key custody",
            "customer-owned coordination server",
            "documented rotation and revocation",
            "scriptable enrollment and recovery",
            "structured audit events for mutating carrier operations",
            "carrier replacement without application redesign",
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn headscale_and_wireguard_are_enterprise_defaults() {
        let profiles = carrier_profiles();
        for id in ["headscale", "wireguard"] {
            let profile = profiles
                .iter()
                .find(|profile| profile.id == id)
                .unwrap_or_else(|| panic!("missing {id}"));
            assert!(
                profile.enterprise_default,
                "{id} must be an enterprise default"
            );
            assert!(profile.authorization_boundary.contains("LinuxMice"));
        }
    }

    #[test]
    fn tailscale_is_optional_carrier_not_trust_root() {
        let tailscale = carrier_profiles()
            .into_iter()
            .find(|profile| profile.id == "tailscale")
            .unwrap();
        assert!(!tailscale.enterprise_default);
        assert!(tailscale.authorization_boundary.contains("LinuxMice"));
    }

    #[test]
    fn policy_is_carrier_untrusted() {
        let policy = carrier_policy();
        assert_eq!(policy.posture, "carrier-untrusted");
        assert!(policy.encryption_rule.contains("above any carrier"));
    }
}
