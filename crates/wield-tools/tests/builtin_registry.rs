use std::collections::BTreeMap;
use wield_core::{render_argv, ArgValue, Capability, Requires};
use wield_tools::builtin_registry;

#[test]
fn color_pick_is_a_valid_portal_tool() {
    let registry = builtin_registry();
    let tool = registry.get("color.pick").expect("color.pick present");
    assert!(matches!(tool.capability, Capability::Portal { .. }));
    assert!(matches!(tool.requires, Requires::Portal { min_ver: 2, .. }));
    assert!(tool.args.is_empty());
}

#[test]
fn image_convert_renders_magick_argv_for_present_and_absent_options() {
    let tool = builtin_registry().get("image.convert").unwrap().clone();
    let Capability::Command(spec) = &tool.capability else {
        panic!("expected Command");
    };

    let mut minimal = BTreeMap::new();
    minimal.insert("input".to_string(), ArgValue::Path("/pics/a.png".into()));
    minimal.insert("format".to_string(), ArgValue::Str("webp".into()));
    let out = std::path::PathBuf::from("/pics/a.webp");
    assert_eq!(
        render_argv(&spec.args, &minimal, Some(&out)).unwrap(),
        vec!["/pics/a.png".to_string(), "/pics/a.webp".into()],
    );

    let mut full = minimal.clone();
    full.insert("width".to_string(), ArgValue::Int(1024));
    full.insert("quality".to_string(), ArgValue::Int(82));
    assert_eq!(
        render_argv(&spec.args, &full, Some(&out)).unwrap(),
        vec![
            "/pics/a.png".to_string(),
            "-resize".into(),
            "1024x".into(),
            "-quality".into(),
            "82".into(),
            "/pics/a.webp".into(),
        ],
    );
}

#[test]
fn registry_has_exactly_the_expected_builtins() {
    let ids: Vec<_> = builtin_registry()
        .list()
        .iter()
        .map(|d| d.id.as_ref().to_string())
        .collect();
    assert_eq!(ids, vec!["color.pick", "image.convert"]);
}

#[test]
fn builtin_registry_matches_snapshot() {
    let actual = builtin_registry().snapshot();
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/snapshots/builtin_registry.json"
    );
    if std::env::var("UPDATE_SNAPSHOTS").is_ok() {
        std::fs::create_dir_all(std::path::Path::new(path).parent().unwrap()).unwrap();
        std::fs::write(path, &actual).unwrap();
        return;
    }
    let expected = std::fs::read_to_string(path)
        .expect("run `UPDATE_SNAPSHOTS=1 cargo test -p wield-tools` to create the snapshot");
    assert_eq!(
        actual, expected,
        "built-in registry changed — review the diff, then UPDATE_SNAPSHOTS=1 to accept"
    );
}
