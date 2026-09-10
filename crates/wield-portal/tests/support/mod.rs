#![allow(dead_code)]

use std::io::{BufRead, BufReader};
use std::process::{Child, Command, Stdio};
use std::time::Duration;
use std::{collections::HashMap, convert::TryFrom};

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

struct PickColorPortal {
    canned: (f64, f64, f64),
    delay: Duration,
}

#[zbus::interface(name = "org.freedesktop.portal.Screenshot")]
impl PickColorPortal {
    #[zbus(property, name = "version")]
    fn version(&self) -> u32 {
        2
    }

    async fn pick_color(
        &self,
        _parent_window: &str,
        options: HashMap<String, zbus::zvariant::OwnedValue>,
        #[zbus(header)] header: zbus::message::Header<'_>,
        #[zbus(connection)] connection: &zbus::Connection,
    ) -> zbus::fdo::Result<zbus::zvariant::OwnedObjectPath> {
        let token = options
            .get("handle_token")
            .and_then(|value| <&str>::try_from(value).ok())
            .ok_or_else(|| zbus::fdo::Error::InvalidArgs("missing handle_token".to_owned()))?;
        let sender = header
            .sender()
            .ok_or_else(|| zbus::fdo::Error::Failed("missing sender".to_owned()))?;
        let sender_component = sender.as_str().trim_start_matches(':').replace('.', "_");
        let path = zbus::zvariant::OwnedObjectPath::try_from(format!(
            "/org/freedesktop/portal/desktop/request/{sender_component}/{token}"
        ))
        .map_err(|error| zbus::fdo::Error::InvalidArgs(error.to_string()))?;

        let connection = connection.clone();
        let response_path = path.clone();
        let canned = self.canned;
        let delay = self.delay;
        tokio::spawn(async move {
            tokio::time::sleep(delay).await;
            let structure = zbus::zvariant::Structure::from(canned);
            let color = zbus::zvariant::OwnedValue::try_from(structure).unwrap();
            let mut results = HashMap::new();
            results.insert("color".to_owned(), color);
            connection
                .emit_signal(
                    None::<()>,
                    response_path,
                    "org.freedesktop.portal.Request",
                    "Response",
                    &(0_u32, results),
                )
                .await
                .unwrap();
        });
        Ok(path)
    }
}

/// Serve a fake Screenshot portal whose PickColor completes asynchronously.
pub async fn serve_fake_pick_color(
    connection: &zbus::Connection,
    canned: (f64, f64, f64),
    delay: Duration,
) {
    connection
        .object_server()
        .at(PORTAL_PATH, PickColorPortal { canned, delay })
        .await
        .unwrap();
    connection
        .request_name("org.freedesktop.portal.Desktop")
        .await
        .unwrap();
}
