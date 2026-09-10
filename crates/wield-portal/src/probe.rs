//! Desktop portal capability probing.

use std::collections::HashMap;

const DESTINATION: &str = "org.freedesktop.portal.Desktop";
const OBJECT_PATH: &str = "/org/freedesktop/portal/desktop";
const PORTAL_PREFIX: &str = "org.freedesktop.portal.";

/// Short portal interface names mapped to their advertised versions.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PortalMap(HashMap<String, u32>);

impl PortalMap {
    /// Return the advertised version of a short interface name.
    pub fn get(&self, iface_short: &str) -> Option<u32> {
        self.0.get(iface_short).copied()
    }

    /// Iterate over short interface names and versions.
    pub fn iter(&self) -> impl Iterator<Item = (&str, u32)> {
        self.0.iter().map(|(name, version)| (name.as_str(), *version))
    }

    /// Whether no portal interfaces were discovered.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Add these portal versions to an existing availability view.
    pub fn apply_to(&self, view: &mut wield_core::AvailabilityView) {
        view.portals
            .extend(self.iter().map(|(name, version)| (name.to_owned(), version)));
    }
}

/// Parse portal interface names from introspection XML, recording version zero.
pub fn parse_portal_versions(introspect_xml: &str) -> PortalMap {
    let Ok(node) = zbus_xml::Node::from_reader(introspect_xml.as_bytes()) else {
        return PortalMap::default();
    };
    let versions = node
        .interfaces()
        .iter()
        .filter_map(|interface| {
            interface
                .name()
                .as_str()
                .strip_prefix(PORTAL_PREFIX)
                .map(|short| (short.to_owned(), 0))
        })
        .collect();
    PortalMap(versions)
}

/// Probe a supplied bus connection. Public so integration harnesses can use a
/// private bus without mutating process-wide environment state.
pub async fn probe_on(connection: &zbus::Connection) -> PortalMap {
    let Ok(introspectable) = zbus::Proxy::new(
        connection,
        DESTINATION,
        OBJECT_PATH,
        "org.freedesktop.DBus.Introspectable",
    )
    .await
    else {
        return PortalMap::default();
    };
    let Ok(xml) = introspectable.call::<_, _, String>("Introspect", &()).await else {
        return PortalMap::default();
    };
    let mut map = parse_portal_versions(&xml);
    let interfaces = map
        .0
        .keys()
        .map(|short| format!("{PORTAL_PREFIX}{short}"))
        .collect::<Vec<_>>();
    for interface in interfaces {
        let Ok(proxy) = zbus::Proxy::new(connection, DESTINATION, OBJECT_PATH, interface.as_str()).await
        else {
            continue;
        };
        if let Ok(version) = proxy.get_property::<u32>("version").await {
            if let Some(short) = interface.strip_prefix(PORTAL_PREFIX) {
                map.0.insert(short.to_owned(), version);
            }
        }
    }
    map
}

/// Probe the session bus. Failures yield an empty map and never block startup.
pub async fn probe() -> PortalMap {
    let Ok(connection) = zbus::Connection::session().await else {
        return PortalMap::default();
    };
    probe_on(&connection).await
}
