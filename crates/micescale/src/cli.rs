use clap::{Parser, Subcommand};
use micescale_core::config::SUPPORTED_CARRIERS;

#[derive(Debug, Parser)]
#[command(
    name = "micescale",
    version,
    about = "Self-hosted Headscale and WireGuard carrier operations for LinuxMice"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Print the version.
    Version,
    /// Report the carrier-untrusted transport policy.
    Carrier {
        #[command(subcommand)]
        action: CarrierAction,
    },
    /// Show non-secret local configuration.
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },
    /// Join a self-hosted tailnet with a pre-auth key (headscale carrier).
    Enroll {
        /// Headscale or WireGuard control server URL.
        #[arg(long)]
        server: String,
        /// Pre-auth key from the headscale server (or MICESCALE_AUTHKEY).
        #[arg(long, env = "MICESCALE_AUTHKEY", hide_env_values = true)]
        authkey: String,
        /// Optional node name visible to peers.
        #[arg(long)]
        node_name: Option<String>,
        /// Carrier profile to configure.
        #[arg(long, default_value = "headscale", value_parser = clap::builder::PossibleValuesParser::new(SUPPORTED_CARRIERS))]
        carrier: String,
    },
    /// Operate a plain WireGuard mesh with no coordination server.
    Wg {
        #[command(subcommand)]
        action: WgAction,
    },
    /// Report tailnet membership and LinuxMice posture.
    Status {
        #[arg(long, default_value = "text", value_parser = ["text", "json", "yaml"])]
        format: String,
    },
    /// Run carrier health checks.
    Doctor {
        /// Optional peer (DNS name or IP) to verify connectivity to.
        #[arg(long)]
        peer: Option<String>,
        #[arg(long, default_value = "text", value_parser = ["text", "json", "yaml"])]
        format: String,
    },
    /// Leave the tailnet.
    Leave,
    /// Inspect local audit events.
    Audit {
        #[command(subcommand)]
        action: AuditAction,
    },
}

#[derive(Debug, Subcommand)]
pub enum WgAction {
    /// Initialize the hub (coordination-free server) keypair and config.
    HubInit {
        #[arg(long, default_value_t = micescale_core::wg::DEFAULT_PORT)]
        listen_port: u16,
        /// Hub address with prefix length, e.g. 10.60.0.1/24.
        #[arg(long)]
        address: String,
        /// Public endpoint reachable by clients, e.g. hub.example.com:51820.
        #[arg(long)]
        endpoint: String,
        #[arg(long, default_value = micescale_core::wg::DEFAULT_INTERFACE)]
        interface: String,
    },
    /// Register a client peer on the hub.
    HubAddPeer {
        #[arg(long)]
        name: String,
        /// Client public key (from `micescale wg client-init`).
        #[arg(long)]
        pubkey: String,
        /// Client address, e.g. 10.60.0.2.
        #[arg(long)]
        address: String,
    },
    /// Remove a client peer from the hub.
    HubRemovePeer {
        #[arg(long)]
        name: String,
    },
    /// Re-render the hub config from the peer registry.
    HubRender,
    /// Generate a client keypair and config for a hub.
    ClientInit {
        /// Client address with prefix length, e.g. 10.60.0.2/24.
        #[arg(long)]
        address: String,
        /// Hub public endpoint, e.g. hub.example.com:51820.
        #[arg(long)]
        endpoint: String,
        /// Hub public key (from `micescale wg hub-init`).
        #[arg(long)]
        hub_pubkey: String,
        /// Routes to send over the tunnel, e.g. 10.60.0.0/24.
        #[arg(long)]
        allowed_ips: String,
        #[arg(long, default_value = micescale_core::wg::DEFAULT_INTERFACE)]
        interface: String,
    },
    /// Bring the interface up (requires root).
    Up,
    /// Bring the interface down (requires root).
    Down,
    /// Report wireguard mesh status.
    Status {
        #[arg(long, default_value = "text", value_parser = ["text", "json", "yaml"])]
        format: String,
    },
}

#[derive(Debug, Subcommand)]
pub enum CarrierAction {
    /// Report the carrier-untrusted posture.
    Policy {
        #[arg(long, default_value = "text", value_parser = ["text", "json", "yaml"])]
        format: String,
    },
    /// List carrier profiles.
    Profiles {
        #[arg(long, default_value = "text", value_parser = ["text", "json", "yaml"])]
        format: String,
    },
}

#[derive(Debug, Subcommand)]
pub enum ConfigAction {
    /// Show non-secret local configuration.
    Show {
        #[arg(long, default_value = "text", value_parser = ["text", "json", "yaml"])]
        format: String,
    },
}

#[derive(Debug, Subcommand)]
pub enum AuditAction {
    /// Show the most recent audit events.
    Tail {
        #[arg(long, default_value_t = 10)]
        limit: usize,
        #[arg(long, default_value = "text", value_parser = ["text", "json", "yaml"])]
        format: String,
    },
}

pub fn parse() -> Cli {
    Cli::parse()
}
