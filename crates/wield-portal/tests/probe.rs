use wield_portal::probe::parse_portal_versions;

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
