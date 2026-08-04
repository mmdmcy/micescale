use std::env;
use std::path::PathBuf;

pub const CONFIG_ENV: &str = "MICESCALE_CONFIG";
pub const AUDIT_ENV: &str = "MICESCALE_AUDIT_LOG";
pub const TAILSCALE_BIN_ENV: &str = "MICESCALE_TAILSCALE_BIN";
pub const AUTHKEY_ENV: &str = "MICESCALE_AUTHKEY";

pub fn default_config_path() -> PathBuf {
    config_dir().join("config.toml")
}

pub fn default_audit_path() -> PathBuf {
    state_dir().join("audit.jsonl")
}

pub fn config_dir() -> PathBuf {
    env::var_os("XDG_CONFIG_HOME")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| home_dir().join(".config"))
        .join("micescale")
}

pub fn state_dir() -> PathBuf {
    env::var_os("XDG_STATE_HOME")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| home_dir().join(".local").join("state"))
        .join("micescale")
}

pub fn home_dir() -> PathBuf {
    env::var_os("HOME")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_env_overrides_default() {
        let base = env::temp_dir().join("micescale-path-test");
        let previous = env::var_os("XDG_CONFIG_HOME");
        // SAFETY: single-threaded test; environment is restored below.
        unsafe { env::set_var("XDG_CONFIG_HOME", &base) };
        let path = default_config_path();
        assert!(path.starts_with(&base));
        assert!(path.ends_with("config.toml"));
        match previous {
            Some(value) => unsafe { env::set_var("XDG_CONFIG_HOME", value) },
            None => unsafe { env::remove_var("XDG_CONFIG_HOME") },
        }
    }
}
