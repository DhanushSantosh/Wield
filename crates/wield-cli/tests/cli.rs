use wield_core::BinaryResolver;

fn args(items: &[&str]) -> Vec<String> {
    items.iter().map(|s| s.to_string()).collect()
}

#[tokio::test]
async fn bare_invocation_lists_the_builtins() {
    assert_eq!(wield_cli::run(args(&["wield"])).await, 0);
}

#[tokio::test]
async fn version_flag() {
    assert_eq!(wield_cli::run(args(&["wield", "--version"])).await, 0);
}

#[tokio::test]
async fn unknown_tool_is_usage_error() {
    assert_eq!(wield_cli::run(args(&["wield", "no.such.tool"])).await, 2);
}

#[tokio::test]
async fn image_convert_missing_input_is_usage_error() {
    // `input` is required; validate_args in the executor returns Failed(Validation),
    // rendered as code 1 — but no positional at all is caught earlier? No: the
    // parser allows an empty positional; the executor rejects the missing required
    // arg. Either way it must be non-zero.
    let code = wield_cli::run(args(&["wield", "image.convert", "--format", "png"])).await;
    assert_ne!(code, 0);
}

#[tokio::test]
async fn image_convert_end_to_end_against_magick() {
    if BinaryResolver::from_env().resolve("magick").is_none() {
        eprintln!("skipping: magick not installed");
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("src.png");
    let made = std::process::Command::new("magick")
        .args(["-size", "3x3", "xc:red", input.to_str().unwrap()])
        .status()
        .unwrap();
    assert!(made.success());

    let code = wield_cli::run(args(&[
        "wield",
        "image.convert",
        input.to_str().unwrap(),
        "--format",
        "webp",
        "--width",
        "2",
    ]))
    .await;
    assert_eq!(code, 0);
    assert!(dir.path().join("src.webp").exists());
}
