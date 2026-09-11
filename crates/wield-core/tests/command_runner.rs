mod support;

use std::time::Duration;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use wield_core::command::{BinaryResolver, CommandResult, CommandRunner, OutputPlan, RunSpec};
use wield_core::descriptor::{ProgressSpec, SuccessSpec};

#[test]
fn resolves_a_binary_on_a_custom_dir() {
    let dir = tempfile::tempdir().unwrap();
    let script = support::write_stub_script(dir.path(), "faketool", "#!/bin/sh\necho hi\n");
    let resolver = BinaryResolver::with_dirs(vec![dir.path().to_path_buf()]);
    assert_eq!(resolver.resolve("faketool"), Some(script));
    assert_eq!(resolver.resolve("definitely-missing-xyz"), None);
}

#[tokio::test]
async fn writes_output_atomically_on_success() {
    let dir = tempfile::tempdir().unwrap();
    let script = support::write_stub_script(
        dir.path(),
        "conv",
        "#!/bin/sh\nprintf 'converted' > \"$2\"\n",
    );
    let final_path = dir.path().join("result.out");
    let plan = OutputPlan::for_final(final_path.clone());
    let (tx, mut rx) = mpsc::channel(16);
    let argv = vec![
        "ignored".to_string(),
        plan.temp.to_string_lossy().into_owned(),
    ];
    let result = CommandRunner::execute(RunSpec {
        binary: &script,
        argv: &argv,
        cwd: None,
        output: Some(&plan),
        progress_spec: &ProgressSpec::None,
        success: &SuccessSpec::ExitZero,
        timeout: Duration::from_secs(5),
        progress: tx,
        cancel: CancellationToken::new(),
    })
    .await;

    assert!(
        matches!(result, CommandResult::Success { output_file: Some(ref path) } if *path == final_path)
    );
    assert_eq!(std::fs::read_to_string(&final_path).unwrap(), "converted");
    assert!(!plan.temp.exists(), "temp file must be gone after rename");

    let mut seen = vec![];
    while let Ok(progress) = rx.try_recv() {
        seen.push(progress);
    }
    assert_eq!(seen.first(), Some(&wield_core::outcome::Progress::Started));
    assert_eq!(seen.last(), Some(&wield_core::outcome::Progress::Finished));
}

#[tokio::test]
async fn nonzero_exit_reports_stderr_tail_and_cleans_temp() {
    let dir = tempfile::tempdir().unwrap();
    let plan = OutputPlan::for_final(dir.path().join("x.out"));
    std::fs::write(&plan.temp, b"partial").unwrap();
    let (tx, _rx) = mpsc::channel(16);
    let argv = vec![
        "-c".to_owned(),
        "echo 'no decode delegate for FOO' 1>&2; exit 3".to_owned(),
    ];
    let result = CommandRunner::execute(RunSpec {
        binary: std::path::Path::new("/bin/sh"),
        argv: &argv,
        cwd: None,
        output: Some(&plan),
        progress_spec: &ProgressSpec::None,
        success: &SuccessSpec::ExitZero,
        timeout: Duration::from_secs(5),
        progress: tx,
        cancel: CancellationToken::new(),
    })
    .await;

    match result {
        CommandResult::NonZeroExit { code, stderr_tail } => {
            assert_eq!(code, Some(3));
            assert!(stderr_tail.contains("no decode delegate"));
        }
        other => panic!("expected NonZeroExit, got {other:?}"),
    }
    assert!(!plan.temp.exists(), "temp must be cleaned on failure");
}

#[tokio::test]
async fn kills_on_timeout() {
    let dir = tempfile::tempdir().unwrap();
    let script = support::write_stub_script(dir.path(), "slow", "#!/bin/sh\nsleep 30\n");
    let plan = OutputPlan::for_final(dir.path().join("x.out"));
    std::fs::write(&plan.temp, b"partial").unwrap();
    let (tx, _rx) = mpsc::channel(16);
    let argv: Vec<String> = vec![];
    let start = std::time::Instant::now();
    let result = CommandRunner::execute(RunSpec {
        binary: &script,
        argv: &argv,
        cwd: None,
        output: Some(&plan),
        progress_spec: &ProgressSpec::None,
        success: &SuccessSpec::ExitZero,
        timeout: Duration::from_millis(300),
        progress: tx,
        cancel: CancellationToken::new(),
    })
    .await;
    assert!(matches!(result, CommandResult::Timeout));
    // Generous upper bound: 300ms timeout + up to 3s SIGTERM grace + scheduler
    // jitter under a loaded CI runner. This only needs to catch a real hang.
    assert!(start.elapsed() < Duration::from_secs(10));
    assert!(!plan.temp.exists());
}

#[tokio::test]
async fn cancels_promptly() {
    let dir = tempfile::tempdir().unwrap();
    let script = support::write_stub_script(dir.path(), "slow2", "#!/bin/sh\nsleep 30\n");
    let (tx, _rx) = mpsc::channel(16);
    let cancel = CancellationToken::new();
    let argv: Vec<String> = vec![];
    let cancellation = cancel.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(200)).await;
        cancellation.cancel();
    });
    // Fail fast if cancellation ever regresses into a hang, rather than
    // blocking on the 30s command timeout.
    let result = tokio::time::timeout(
        Duration::from_secs(10),
        CommandRunner::execute(RunSpec {
            binary: &script,
            argv: &argv,
            cwd: None,
            output: None,
            progress_spec: &ProgressSpec::None,
            success: &SuccessSpec::ExitZero,
            timeout: Duration::from_secs(30),
            progress: tx,
            cancel,
        }),
    )
    .await
    .expect("cancellation should resolve well before the command timeout");
    assert!(matches!(result, CommandResult::Cancelled));
}

#[tokio::test]
async fn emits_started_then_finished_for_progress_none() {
    let dir = tempfile::tempdir().unwrap();
    let script = support::write_stub_script(dir.path(), "quick", "#!/bin/sh\nexit 0\n");
    let (tx, mut rx) = mpsc::channel(16);
    let argv: Vec<String> = vec![];
    let _result = CommandRunner::execute(RunSpec {
        binary: &script,
        argv: &argv,
        cwd: None,
        output: None,
        progress_spec: &ProgressSpec::None,
        success: &SuccessSpec::ExitZero,
        timeout: Duration::from_secs(5),
        progress: tx,
        cancel: CancellationToken::new(),
    })
    .await;
    let mut seen = vec![];
    while let Some(progress) = rx.recv().await {
        seen.push(progress);
    }
    assert_eq!(
        seen,
        vec![
            wield_core::outcome::Progress::Started,
            wield_core::outcome::Progress::Finished,
        ]
    );
}
