use wield_portal::probe::parse_portal_versions;

mod support;

const SAMPLE: &str = r#"<!DOCTYPE node PUBLIC "-//freedesktop//DTD D-BUS Object Introspection 1.0//EN" "http://www.freedesktop.org/standards/dbus/1.0/introspect.dtd">
<node>
  <interface name="org.freedesktop.DBus.Introspectable">
    <method name="Introspect"><arg type="s" direction="out"/></method>
  </interface>
  <interface name="org.freedesktop.portal.Screenshot">
    <method name="PickColor"><arg type="s"/><arg type="a{sv}"/><arg type="o" direction="out"/></method>
    <property name="version" type="u" access="read"/>
  </interface>
  <interface name="org.freedesktop.portal.Inhibit">
    <property name="version" type="u" access="read"/>
  </interface>
</node>"#;

#[test]
fn parses_portal_interface_names() {
    let map = parse_portal_versions(SAMPLE);
    assert_eq!(map.get("Screenshot"), Some(0));
    assert_eq!(map.get("Inhibit"), Some(0));
    assert_eq!(map.get("Introspectable"), None);
    assert_eq!(map.get("GlobalShortcuts"), None);
}

#[tokio::test]
async fn probe_reads_versions_from_a_fake_portal() {
    let Some(bus) = support::PrivateBus::launch() else {
        eprintln!("skipping: dbus-daemon not available");
        return;
    };
    let server = bus.connect().await;
    support::serve_fake_portal(&server, &[("Screenshot", 2), ("Inhibit", 3)]).await;

    let client = bus.connect().await;
    let map = wield_portal::probe::probe_on(&client).await;

    assert_eq!(map.get("Screenshot"), Some(2));
    assert_eq!(map.get("Inhibit"), Some(3));
    assert_eq!(map.get("GlobalShortcuts"), None);
}
