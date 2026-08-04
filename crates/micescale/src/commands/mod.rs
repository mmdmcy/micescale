pub mod audit;
pub mod carrier;
pub mod config;
pub mod doctor;
pub mod enroll;
pub mod leave;
pub mod status;
pub mod wg;

use crate::cli::{AuditAction, CarrierAction, Cli, Command, ConfigAction, WgAction};
pub use crate::error::AppError;

pub fn run(cli: Cli) -> Result<(), AppError> {
    match cli.command {
        Command::Version => {
            println!("{} {}", micescale_core::PRODUCT, env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        Command::Carrier { action } => match action {
            CarrierAction::Policy { format } => carrier::policy(&format),
            CarrierAction::Profiles { format } => carrier::profiles(&format),
        },
        Command::Config { action } => match action {
            ConfigAction::Show { format } => config::show(&format),
        },
        Command::Enroll {
            server,
            authkey,
            node_name,
            carrier,
        } => enroll::run(&server, &authkey, node_name.as_deref(), &carrier),
        Command::Wg { action } => match action {
            WgAction::HubInit {
                listen_port,
                address,
                endpoint,
                interface,
            } => wg::hub_init(listen_port, &address, &endpoint, &interface),
            WgAction::HubAddPeer {
                name,
                pubkey,
                address,
            } => wg::hub_add_peer(&name, &pubkey, &address),
            WgAction::HubRemovePeer { name } => wg::hub_remove_peer(&name),
            WgAction::HubRender => wg::hub_render(),
            WgAction::ClientInit {
                address,
                endpoint,
                hub_pubkey,
                allowed_ips,
                interface,
            } => wg::client_init(&address, &endpoint, &hub_pubkey, &allowed_ips, &interface),
            WgAction::Up => wg::up(),
            WgAction::Down => wg::down(),
            WgAction::Status { format } => wg::status(&format),
        },
        Command::Status { format } => status::run(&format),
        Command::Doctor { peer, format } => doctor::run(peer.as_deref(), &format),
        Command::Leave => leave::run(),
        Command::Audit { action } => match action {
            AuditAction::Tail { limit, format } => audit::tail(limit, &format),
        },
    }
}
