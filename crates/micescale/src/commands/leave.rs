use micescale_core::config;

use crate::AppError;
use crate::commands::enroll::append_event;
use crate::tailscale;

pub fn run() -> Result<(), AppError> {
    let path = config::default_path();
    let config = config::load(&path)?;
    match config {
        Some(config) if config.carrier == "wireguard" => return crate::commands::wg::leave(),
        Some(config) => {
            tailscale::logout()?;
            append_event(&config, "leave", "ok", Some("logged out of tailnet"))?;
        }
        None => {
            tailscale::logout()?;
        }
    }
    println!("left the tailnet");
    Ok(())
}
