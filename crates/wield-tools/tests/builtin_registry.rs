use wield_core::{Capability, Requires};
use wield_tools::builtin_registry;

#[test]
fn color_pick_is_a_valid_portal_tool() {
    let registry = builtin_registry();
    let tool = registry.get("color.pick").expect("color.pick present");
    assert!(matches!(tool.capability, Capability::Portal { .. }));
    assert!(matches!(tool.requires, Requires::Portal { min_ver: 2, .. }));
    assert!(tool.args.is_empty());
}
