//! Send notifications with `notify-send`.

use std::process::Command;

use color_eyre::eyre;
use color_eyre::eyre::WrapErr;
use tracing::debug;

pub struct Notification {
    pub title: String,
    pub message: String,
    pub user: Option<String>,
}

impl Notification {
    pub fn send(&self) -> eyre::Result<()> {
        let (cmd_name, args) = match &self.user {
            Some(user) => ("sudo", vec!["-u", &user, "notify-send"]),
            None => ("notify-send", vec![]),
        };

        let output = Command::new(cmd_name)
            .args(args)
            .output()
            .context(format!("Failed to execute {}", cmd_name))?;

        if output.status.success() {
            debug!(
                stdout = %String::from_utf8_lossy(&output.stdout),
                stderr = %String::from_utf8_lossy(&output.stderr),
                "notify-send exited succesfully"
            );
            Ok(())
        } else {
            Err(eyre::eyre!(
                "notify-send failed with exit code {}",
                output.status
            ))
            .context(format!(
                "stderr: {}",
                String::from_utf8_lossy(&output.stderr)
            ))
            .context(format!(
                "stdout: {}",
                String::from_utf8_lossy(&output.stdout)
            ))
        }
    }

    pub fn send_unchecked(&self) {
        if let Err(err) = self.send() {
            tracing::error!(?err, "Failed to send notification");
        }
    }
}
