// SPDX-License-Identifier: Apache-2.0

use std::process::Command;
use std::time::Duration;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum SandboxError {
    #[error("Validation failed with exit code: {0}")]
    ValidationFailed(i32),
    #[error("Sandbox timeout exceeded")]
    Timeout,
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

pub struct Sandbox {
    timeout: Duration,
}

impl Sandbox {
    pub fn new() -> Self {
        Self {
            timeout: Duration::from_secs(5),
        }
    }

    pub fn validate_code(&self, code: &str) -> Result<(), SandboxError> {
        use std::io::Write;
        use tempfile::NamedTempFile;

        let mut temp_file = NamedTempFile::new()?;
        temp_file.write_all(code.as_bytes())?;

        // Wait, rcrun might not be in PATH if it's a binary from this repo.
        // Usually, the CLI binary is compiled to target/debug/cobolt.
        // We will just invoke "cargo" "run" "--bin" "cobolt" "--" "check" "--pedantic" <file>
        // Or if rcrun is the actual command, we use "rcrun".
        // Let's assume the user has the binary built or "cargo run" works.
        // For testing, "cargo" "run" "--bin" "cobolt" "--" "check" "--pedantic" is safe.
        let output = Command::new("cargo")
            .arg("run")
            .arg("--bin")
            .arg("cobolt")
            .arg("--")
            .arg("check")
            .arg("--pedantic")
            .arg(temp_file.path())
            .output()?;

        if !output.status.success() {
            return Err(SandboxError::ValidationFailed(
                output.status.code().unwrap_or(-1),
            ));
        }

        Ok(())
    }
}
