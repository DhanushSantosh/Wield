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
