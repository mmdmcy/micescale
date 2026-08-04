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
    /// Join a self-hosted tailnet with a pre-auth key.
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
