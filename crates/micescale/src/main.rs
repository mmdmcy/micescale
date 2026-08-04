mod cli;
mod commands;
mod error;
mod tailscale;

pub use error::AppError;

fn main() {
    let parsed = cli::parse();
    if let Err(error) = commands::run(parsed) {
        eprintln!("micescale: {error}");
        std::process::exit(error.exit_code());
    }
}
