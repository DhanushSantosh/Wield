use std::io::{BufRead, BufReader};
use std::process::{Child, Command, Stdio};

pub struct PrivateBus {
    address: String,
    child: Child,
    _directory: tempfile::TempDir,
}

impl PrivateBus {
    pub fn launch() -> Option<Self> {
        let directory = tempfile::tempdir().ok()?;
        let socket = directory.path().join("session-bus");
        let mut child = Command::new("dbus-daemon")
            .args([
                "--session",
                "--nofork",
                "--print-address=1",
                &format!("--address=unix:path={}", socket.display()),
            ])
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .ok()?;
        let stdout = child.stdout.take()?;
        let mut address = String::new();
        BufReader::new(stdout).read_line(&mut address).ok()?;
        let address = address.trim().to_owned();
        if address.is_empty() {
            let _ = child.kill();
            return None;
        }
        Some(Self {
            address,
            child,
            _directory: directory,
        })
    }

    pub async fn connect(&self) -> zbus::Result<zbus::Connection> {
        zbus::connection::Builder::address(self.address.as_str())?
            .build()
            .await
    }
}

impl Drop for PrivateBus {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
