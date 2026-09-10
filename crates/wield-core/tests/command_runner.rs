mod support;

use wield_core::command::BinaryResolver;

#[test]
fn resolves_a_binary_on_a_custom_dir() {
    let dir = tempfile::tempdir().unwrap();
    let script = support::write_stub_script(
        dir.path(),
        "faketool",
        "#!/bin/sh\necho hi\n",
    );
    let resolver = BinaryResolver::with_dirs(vec![dir.path().to_path_buf()]);
    assert_eq!(resolver.resolve("faketool"), Some(script));
    assert_eq!(resolver.resolve("definitely-missing-xyz"), None);
}
