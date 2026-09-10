#![allow(dead_code)]

use std::io::{BufRead, BufReader};
use std::process::{Child, Command, Stdio};

const PORTAL_PATH: &str = "/org/freedesktop/portal/desktop";

/// A throwaway session bus backed by its own `dbus-daemon`.
pub struct PrivateBus {
    address: String,
    child: Child,
    _directory: tempfile::TempDir,
}

impl PrivateBus {
    /// Launch a private bus, returning `None` when `dbus-daemon` is unavailable.
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

    pub fn address(&self) -> &str {
        &self.address
    }

    /// Connect to this private bus.
    pub async fn connect(&self) -> zbus::Connection {
        zbus::connection::Builder::address(self.address.as_str())
            .unwrap()
            .build()
            .await
            .unwrap()
    }
}

impl Drop for PrivateBus {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

struct Screenshot(u32);

#[zbus::interface(name = "org.freedesktop.portal.Screenshot")]
impl Screenshot {
    #[zbus(property, name = "version")]
    fn version(&self) -> u32 {
        self.0
    }
}

struct Inhibit(u32);

#[zbus::interface(name = "org.freedesktop.portal.Inhibit")]
impl Inhibit {
    #[zbus(property, name = "version")]
    fn version(&self) -> u32 {
        self.0
    }
}

/// Serve the portal interfaces needed by the integration tests.
pub async fn serve_fake_portal(connection: &zbus::Connection, ifaces: &[(&str, u32)]) {
    for (name, version) in ifaces {
        match *name {
            "Screenshot" => {
                connection
                    .object_server()
                    .at(PORTAL_PATH, Screenshot(*version))
                    .await
                    .unwrap();
            }
            "Inhibit" => {
                connection
                    .object_server()
                    .at(PORTAL_PATH, Inhibit(*version))
                    .await
                    .unwrap();
            }
            other => panic!("unsupported fake portal interface: {other}"),
        }
    }
    connection
        .request_name("org.freedesktop.portal.Desktop")
        .await
        .unwrap();
}
