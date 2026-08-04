use std::process::Command;

use crate::AppError;

pub struct Output {
    pub status: Option<i32>,
    pub stdout: String,
    pub stderr: String,
}

impl Output {
    pub fn ok(&self, context: &str) -> Result<(), AppError> {
        if self.status == Some(0) {
            Ok(())
        } else {
            let detail = if self.stderr.trim().is_empty() {
                self.stdout.trim()
            } else {
                self.stderr.trim()
            };
            Err(AppError::Operational(format!("{context} failed: {detail}")))
        }
    }
}

pub fn run(binary: &str, args: &[&str]) -> Result<Output, AppError> {
    let output = Command::new(binary)
        .args(args)
        .output()
        .map_err(|error| AppError::Operational(format!("cannot execute {binary}: {error}")))?;
    Ok(Output {
        status: output.status.code(),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    })
}

/// Run a command with `input` written to stdin. Used for `wg pubkey`.
pub fn run_with_input(binary: &str, args: &[&str], input: &str) -> Result<Output, AppError> {
    use std::io::Write;
    use std::process::Stdio;
    let mut child = Command::new(binary)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| AppError::Operational(format!("cannot execute {binary}: {error}")))?;
    child
        .stdin
        .as_mut()
        .expect("stdin piped")
        .write_all(input.as_bytes())
        .map_err(|error| AppError::Operational(format!("cannot write to {binary}: {error}")))?;
    let output = child
        .wait_with_output()
        .map_err(|error| AppError::Operational(format!("cannot wait for {binary}: {error}")))?;
    Ok(Output {
        status: output.status.code(),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    })
}
