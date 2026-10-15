use std::fs::{self, OpenOptions};
use std::io::Write;
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;

use crate::AppError;

/// Write a secret file atomically: the content goes to a sibling temp file
/// created 0600 (never readable by others, even briefly) and is renamed over
/// `path`, so readers see either the old file or the complete new one.
pub fn write_private(path: &Path, content: &str) -> Result<(), AppError> {
    let name = path
        .file_name()
        .ok_or_else(|| AppError::Operational(format!("{} has no file name", path.display())))?;
    let tmp = path.with_file_name(format!(".{}.tmp", name.to_string_lossy()));
    let _ = fs::remove_file(&tmp);
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&tmp)
        .map_err(|error| {
            AppError::Operational(format!("cannot create {}: {error}", tmp.display()))
        })?;
    file.write_all(content.as_bytes())
        .and_then(|()| file.sync_all())
        .map_err(|error| {
            AppError::Operational(format!("cannot write {}: {error}", tmp.display()))
        })?;
    fs::rename(&tmp, path)
        .map_err(|error| AppError::Operational(format!("cannot write {}: {error}", path.display())))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn private_files_are_0600_even_when_replacing_a_wider_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("secret.conf");
        fs::write(&path, "old").unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
        write_private(&path, "new").unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), "new");
        let mode = fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
        assert!(!dir.path().join(".secret.conf.tmp").exists());
    }
}
