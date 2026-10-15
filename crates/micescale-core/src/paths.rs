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

/// The invoking user's home. Under `sudo` (USER=root, SUDO_USER set) this is
/// SUDO_USER's home from /etc/passwd, so `sudo micescale wg up` finds the
/// state written by the unprivileged user instead of looking under /root.
pub fn home_dir() -> PathBuf {
    let sudo_user = env::var("SUDO_USER").ok().filter(|user| {
        !user.is_empty() && user != "root" && env::var("USER").as_deref() == Ok("root")
    });
    if let Some(user) = sudo_user {
        let passwd = std::fs::read_to_string("/etc/passwd").unwrap_or_default();
        if let Some(home) = passwd_home(&passwd, &user) {
            return home;
        }
    }
    env::var_os("HOME")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
}

/// Home directory of `user` in /etc/passwd format (name:pw:uid:gid:gecos:home:shell).
pub fn passwd_home(passwd: &str, user: &str) -> Option<PathBuf> {
    passwd.lines().find_map(|line| {
        let fields: Vec<&str> = line.split(':').collect();
        (fields.len() >= 7 && fields[0] == user && !fields[5].is_empty())
            .then(|| PathBuf::from(fields[5]))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn passwd_home_finds_the_sudo_user() {
        let passwd = "root:x:0:0:root:/root:/bin/bash\n# comment\nmac:x:1000:1000:Mac,,,:/home/mac:/bin/bash\nbroken:x:1\n";
        assert_eq!(passwd_home(passwd, "mac"), Some(PathBuf::from("/home/mac")));
        assert_eq!(passwd_home(passwd, "root"), Some(PathBuf::from("/root")));
        assert_eq!(passwd_home(passwd, "ma"), None);
        assert_eq!(passwd_home(passwd, "broken"), None);
    }

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
