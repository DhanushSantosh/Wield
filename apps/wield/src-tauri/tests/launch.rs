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
    std::thread::sleep(std::time::Duration::from_secs(3));

    let owned = std::process::Command::new("busctl")
        .args(["--user", "list"])
        .output()
        .map(|output| {
            String::from_utf8_lossy(&output.stdout).contains("io.github.DhanushSantosh.Wield")
        })
        .unwrap_or(false);

    let _ = child.kill();
    let _ = child.wait();
    assert!(owned, "shell should own the D-Bus name while running");
}
