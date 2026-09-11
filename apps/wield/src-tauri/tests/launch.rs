//! Launch smoke tests require a graphical session and a session D-Bus.

#[test]
#[ignore = "needs a display + session bus; run with --ignored"]
fn shell_launches_headless_and_owns_the_bus_name() {
    if std::env::var("WAYLAND_DISPLAY").is_err() && std::env::var("DISPLAY").is_err() {
        eprintln!("skipping: no display");
        return;
    }
    if std::process::Command::new("busctl")
        .arg("--version")
        .output()
        .is_err()
    {
        eprintln!("skipping: busctl unavailable");
        return;
    }

    let mut child = std::process::Command::new(env!("CARGO_BIN_EXE_wield-app"))
        .spawn()
        .unwrap();

    // Poll rather than a fixed sleep: under a loaded runner (right after a big
    // recompile) the shell can take longer than a few seconds to acquire the
    // bus name, so a flat sleep flakes. Give it up to 15s, checking often.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(15);
    let mut owned = false;
    while std::time::Instant::now() < deadline {
        owned = std::process::Command::new("busctl")
            .args(["--user", "list"])
            .output()
            .map(|output| {
                String::from_utf8_lossy(&output.stdout).contains("io.github.DhanushSantosh.Wield")
            })
            .unwrap_or(false);
        if owned {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(250));
    }

    let _ = child.kill();
    let _ = child.wait();
    assert!(owned, "shell should own the D-Bus name while running");
}

#[test]
#[ignore = "needs a display + session bus + a real `magick` on PATH; run with --ignored"]
fn run_tool_converts_a_real_fixture_end_to_end() {
    if std::env::var("WAYLAND_DISPLAY").is_err() && std::env::var("DISPLAY").is_err() {
        eprintln!("skipping: no display");
        return;
    }
    if std::process::Command::new("magick")
        .arg("-version")
        .output()
        .is_err()
    {
        eprintln!("skipping: magick unavailable");
        return;
    }

    let output_dir = tempfile::tempdir().unwrap();
    let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let source_fixture = manifest_dir
        .join("../../../assets/icon.png")
        .canonicalize()
        .expect("assets/icon.png should exist at the repo root");
    let fixture = output_dir.path().join("fixture.png");
    std::fs::copy(source_fixture, &fixture).expect("fixture should copy into the temp directory");

    let mut child = std::process::Command::new(env!("CARGO_BIN_EXE_wield-app"))
        .env("XDG_STATE_HOME", output_dir.path())
        .spawn()
        .unwrap();

    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(15);
    let mut owned = false;
    while std::time::Instant::now() < deadline {
        owned = std::process::Command::new("busctl")
            .args(["--user", "list"])
            .output()
            .map(|output| {
                String::from_utf8_lossy(&output.stdout).contains("io.github.DhanushSantosh.Wield")
            })
            .unwrap_or(false);
        if owned {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(250));
    }
    assert!(
        owned,
        "shell should own the D-Bus name before RunTool is callable"
    );

    let outcome_json = wield_run_tool_over_dbus(
        "image.convert",
        &serde_json::json!({
            "input": fixture.to_string_lossy(),
            "format": "webp",
        })
        .to_string(),
    );

    let _ = child.kill();
    let _ = child.wait();

    let outcome: serde_json::Value =
        serde_json::from_str(&outcome_json).expect("RunTool should return JSON");
    let path = outcome["File"]["path"]
        .as_str()
        .unwrap_or_else(|| panic!("expected a File outcome, got: {outcome_json}"));
    assert!(
        std::path::Path::new(path).exists(),
        "converted output file should exist at {path}"
    );
}

fn wield_run_tool_over_dbus(id: &str, args_json: &str) -> String {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    runtime.block_on(async {
        let connection = zbus::Connection::session().await.unwrap();
        let proxy = zbus::Proxy::new(
            &connection,
            "io.github.DhanushSantosh.Wield",
            "/io/github/DhanushSantosh/Wield",
            "io.github.DhanushSantosh.Wield",
        )
        .await
        .unwrap();
        proxy
            .call::<_, _, String>("RunTool", &(id, args_json))
            .await
            .unwrap()
    })
}
