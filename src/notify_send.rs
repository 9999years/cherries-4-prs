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
        // There's no good way to find the `DISPLAY` and `DBUS_SESSION_BUS_ADDRESS`
        // for a given user, so we just hope that `DISPLAY=:0` and
        // `DBUS_SESSION_BUS_ADDRESS=unix:path=/run/user/{uid}/bus` works.
        //
        // See: https://unix.stackexchange.com/questions/2881/show-a-notification-across-all-running-x-displays
        //      https://superuser.com/questions/647464/how-to-get-the-display-number-i-was-assigned-by-x

        let (cmd_name, args, envs) = match &self.user {
            Some(user) => (
                "sudo",
                vec![
                    "-u",
                    &user,
                    "--preserve-env=DISPLAY,DBUS_SESSION_BUS_ADDRESS",
                    "notify-send",
                ],
                vec![
                    ("DISPLAY", ":0".to_owned()),
                    ("DBUS_SESSION_BUS_ADDRESS", {
                        let uid = run_command(Command::new("id").args(["-u", user]))?;

                        format!("unix:path=/run/user/{uid}/bus")
                    }),
                ],
            ),
            None => ("notify-send", vec![], vec![]),
        };

        run_command(Command::new(cmd_name).args(args).envs(envs)).map(|_| ())
    }

    pub fn send_unchecked(&self) {
        if let Err(err) = self.send() {
            tracing::error!(?err, "Failed to send notification");
        }
    }
}

fn run_command(cmd: &mut Command) -> eyre::Result<String> {
    let cmd_name = cmd.get_program().to_owned();
    let output = cmd
        .output()
        .context(format!("Failed to execute {cmd_name:?}"))?;

    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        debug!(
            stdout = %stdout,
            stderr = %String::from_utf8_lossy(&output.stderr),
            "{cmd_name:?} exited succesfully",
        );
        Ok(stdout.into_owned())
    } else {
        Err(eyre::eyre!(
            "{cmd_name:?} failed with exit code {}",
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
