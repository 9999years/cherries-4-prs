//! Send notifications with `notify-send`.

use std::process::Command;

use color_eyre::eyre;

pub struct Notification {
    pub title: String,
    pub message: String,
}

impl Notification {
    pub fn send(&self) -> eyre::Result<()> {
        let mut cmd = Command::new("notify-send")
            .args(&[&self.title, &self.message])
            .spawn()?;
        let status = cmd.wait()?;
        if !status.success() {
            return Err(eyre::eyre!("notify-send failed with exit code {}", status));
        }

        Ok(())
    }

    pub fn send_unchecked(&self) {
        if let Err(err) = self.send() {
            tracing::error!(?err, "Failed to send notification");
        }
    }
}
