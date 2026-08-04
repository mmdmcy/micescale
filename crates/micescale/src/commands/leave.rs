use micescale_core::config;

use crate::AppError;
use crate::commands::enroll::append_event;
use crate::tailscale;

pub fn run() -> Result<(), AppError> {
    let path = config::default_path();
    let config = config::load(&path)?;
    tailscale::logout()?;
    if let Some(config) = config {
        append_event(&config, "leave", "ok", Some("logged out of tailnet"))?;
    }
    println!("left the tailnet");
    Ok(())
}
