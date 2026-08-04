pub mod audit;
pub mod carrier;
pub mod config;
pub mod doctor;
pub mod enroll;
pub mod leave;
pub mod status;

use crate::cli::{AuditAction, CarrierAction, Cli, Command, ConfigAction};
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
        Command::Status { format } => status::run(&format),
        Command::Doctor { peer, format } => doctor::run(peer.as_deref(), &format),
        Command::Leave => leave::run(),
        Command::Audit { action } => match action {
            AuditAction::Tail { limit, format } => audit::tail(limit, &format),
        },
    }
}
