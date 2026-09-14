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

#[tokio::test]
async fn ffmpeg_duration_parser_reports_percent_from_real_captured_output() {
    let dir = tempfile::tempdir().unwrap();
    // Exact lines captured live from a real `ffmpeg -progress pipe:2
    // -nostats` run during this feature's own design verification - not
    // synthesized, the real shape ffmpeg actually produces (including the
    // noise this parser must tolerate: the leftover `frame=  200 fps=0.0
    // ... Lsize=...` summary line `-nostats` does NOT suppress). Also
    // includes a leading `out_time_us=N/A` block - ffmpeg's first
    // `-progress` report commonly arrives before it has computed real
    // elapsed time; the parser must tolerate this without emitting a
    // percent.
    let script = r#"#!/bin/sh
cat <<'EOF' 1>&2
ffmpeg version n9.0 Copyright (c) 2000-2026 the FFmpeg developers
Input #0, mov,mp4,m4a,3gp,3g2,mj2, from 'test-input.mp4':
  Duration: 00:00:08.00, start: 0.000000, bitrate: 47 kb/s
Stream mapping:
  Stream #0:0 -> #0:0 (h264 (native) -> h264 (libx264))
frame=1
fps=0.00
stream_0_0_q=0.0
bitrate=N/A
total_size=0
out_time_us=N/A
out_time_ms=N/A
out_time=N/A
dup_frames=0
drop_frames=0
speed=   0x
progress=continue
frame=98
fps=0.00
stream_0_0_q=28.0
bitrate=   0.1kbits/s
total_size=48
out_time_us=3840000
out_time_ms=3840000
out_time=00:00:03.840000
dup_frames=0
drop_frames=0
speed=7.65x
progress=continue
frame=  200 fps=0.0 q=-1.0 Lsize=      45KiB time=00:00:07.92 bitrate=  46.3kbits/s speed=9.97x elapsed=0:00:00.79
frame=200
fps=0.00
stream_0_0_q=-1.0
bitrate=  46.3kbits/s
total_size=45886
out_time_us=7920000
out_time_ms=7920000
out_time=00:00:07.920000
dup_frames=0
drop_frames=0
speed=9.97x
progress=end
EOF
echo done > "$2"
"#;
    let script = support::write_stub_script(dir.path(), "fake-ffmpeg", script);
    let plan = OutputPlan::for_final(dir.path().join("out.mp4"));
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
        progress_spec: &ProgressSpec::FfmpegDuration,
        success: &SuccessSpec::ExitZero,
        timeout: Duration::from_secs(5),
        progress: tx,
        cancel: CancellationToken::new(),
    })
    .await;
    assert!(matches!(result, CommandResult::Success { .. }));

    let mut percents = vec![];
    while let Ok(progress) = rx.try_recv() {
        if let wield_core::outcome::Progress::Percent(p) = progress {
            percents.push(p);
        }
    }
    // 3.84s / 8.00s = 48%, 7.92s / 8.00s = 99% (rounds down from 99.0).
    assert_eq!(percents, vec![48, 99]);
}
