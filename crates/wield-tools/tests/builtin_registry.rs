use std::collections::BTreeMap;
use wield_core::{render_argv, ArgValue, Capability, Requires};
use wield_tools::builtin_registry;

const EXPECTED_SCRIPT: &str = r#"set -e
in="$1"; out="$2"; fmt="$3"
case "$fmt" in
  pdf)
    dir=$(dirname "$out")
    stem=$(basename "$out" ".$fmt")
    ext=$(printf "%s" "$in" | sed "s/^.*\.//")
    tmp="$dir/$stem.$ext"
    trap "rm -f \"$tmp\"" EXIT
    cp "$in" "$tmp"
    soffice --headless --convert-to pdf --outdir "$dir" "$tmp" 1>&2
    ;;
  *)
    pandoc --standalone "$in" -o "$out"
    ;;
esac
"#;

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
fn video_convert_renders_ffmpeg_argv_for_present_and_absent_options() {
    let tool = builtin_registry().get("video.convert").unwrap().clone();
    let Capability::Command(spec) = &tool.capability else {
        panic!("expected Command");
    };

    let mut minimal = BTreeMap::new();
    minimal.insert("input".to_string(), ArgValue::Path("/clips/a.mov".into()));
    minimal.insert("format".to_string(), ArgValue::Str("mp4".into()));
    minimal.insert("resolution".to_string(), ArgValue::Str("original".into()));
    let out = std::path::PathBuf::from("/clips/a.mp4");
    assert_eq!(
        render_argv(&spec.args, &minimal, Some(&out)).unwrap(),
        vec![
            "-y".to_string(),
            "-i".into(),
            "/clips/a.mov".into(),
            "-progress".into(),
            "pipe:2".into(),
            "-nostats".into(),
            "/clips/a.mp4".into(),
        ],
    );

    let mut full = minimal.clone();
    full.insert("resolution".to_string(), ArgValue::Str("720p".into()));
    full.insert("quality".to_string(), ArgValue::Int(23));
    assert_eq!(
        render_argv(&spec.args, &full, Some(&out)).unwrap(),
        vec![
            "-y".to_string(),
            "-i".into(),
            "/clips/a.mov".into(),
            "-vf".into(),
            "scale=-2:720".into(),
            "-crf".into(),
            "23".into(),
            "-progress".into(),
            "pipe:2".into(),
            "-nostats".into(),
            "/clips/a.mp4".into(),
        ],
    );
}

#[test]
fn audio_extract_renders_ffmpeg_argv_for_present_and_absent_options() {
    let tool = builtin_registry().get("audio.extract").unwrap().clone();
    let Capability::Command(spec) = &tool.capability else {
        panic!("expected Command");
    };

    let mut minimal = BTreeMap::new();
    minimal.insert("input".to_string(), ArgValue::Path("/clips/a.mp4".into()));
    minimal.insert("format".to_string(), ArgValue::Str("mp3".into()));
    let out = std::path::PathBuf::from("/clips/a.mp3");
    assert_eq!(
        render_argv(&spec.args, &minimal, Some(&out)).unwrap(),
        vec![
            "-y".to_string(),
            "-i".into(),
            "/clips/a.mp4".into(),
            "-vn".into(),
            "-codec:a".into(),
            "libmp3lame".into(),
            "-progress".into(),
            "pipe:2".into(),
            "-nostats".into(),
            "/clips/a.mp3".into(),
        ],
    );

    let mut full = minimal.clone();
    full.insert("quality".to_string(), ArgValue::Str("256k".into()));
    assert_eq!(
        render_argv(&spec.args, &full, Some(&out)).unwrap(),
        vec![
            "-y".to_string(),
            "-i".into(),
            "/clips/a.mp4".into(),
            "-vn".into(),
            "-codec:a".into(),
            "libmp3lame".into(),
            "-b:a".into(),
            "256k".into(),
            "-progress".into(),
            "pipe:2".into(),
            "-nostats".into(),
            "/clips/a.mp3".into(),
        ],
    );
}

#[test]
fn audio_extract_picks_the_right_codec_for_each_format() {
    let tool = builtin_registry().get("audio.extract").unwrap().clone();
    let Capability::Command(spec) = &tool.capability else {
        panic!("expected Command");
    };
    let out = std::path::PathBuf::from("/clips/a.out");

    for (format, codec) in [
        ("mp3", "libmp3lame"),
        ("m4a", "aac"),
        ("flac", "flac"),
        ("wav", "pcm_s16le"),
    ] {
        let mut args = BTreeMap::new();
        args.insert("input".to_string(), ArgValue::Path("/clips/a.mp4".into()));
        args.insert("format".to_string(), ArgValue::Str(format.to_string()));
        let argv = render_argv(&spec.args, &args, Some(&out)).unwrap();
        assert!(
            argv.windows(2).any(|pair| pair == ["-codec:a", codec]),
            "format {format}: expected -codec:a {codec} in {argv:?}"
        );
        for (_, other_codec) in [
            ("mp3", "libmp3lame"),
            ("m4a", "aac"),
            ("flac", "flac"),
            ("wav", "pcm_s16le"),
        ] {
            if other_codec == codec {
                continue;
            }
            assert!(
                !argv.iter().any(|segment| segment == other_codec),
                "format {format}: unexpected codec {other_codec} leaked into {argv:?}"
            );
        }
    }
}

#[test]
fn document_convert_renders_the_pandoc_direct_branch() {
    let tool = builtin_registry().get("document.convert").unwrap().clone();
    let Capability::Command(spec) = &tool.capability else {
        panic!("expected Command");
    };

    let mut args = BTreeMap::new();
    args.insert("input".to_string(), ArgValue::Path("/docs/a.md".into()));
    args.insert("format".to_string(), ArgValue::Str("docx".into()));
    let out = std::path::PathBuf::from("/docs/a.docx");
    assert_eq!(
        render_argv(&spec.args, &args, Some(&out)).unwrap(),
        vec![
            "-c".to_string(),
            EXPECTED_SCRIPT.to_string(),
            "sh".into(),
            "/docs/a.md".into(),
            "/docs/a.docx".into(),
            "docx".into(),
        ],
    );
}

#[test]
fn document_convert_renders_the_pdf_wrapper_branch() {
    let tool = builtin_registry().get("document.convert").unwrap().clone();
    let Capability::Command(spec) = &tool.capability else {
        panic!("expected Command");
    };

    let mut args = BTreeMap::new();
    args.insert("input".to_string(), ArgValue::Path("/docs/a.md".into()));
    args.insert("format".to_string(), ArgValue::Str("pdf".into()));
    let out = std::path::PathBuf::from("/docs/a.pdf");
    assert_eq!(
        render_argv(&spec.args, &args, Some(&out)).unwrap(),
        vec![
            "-c".to_string(),
            EXPECTED_SCRIPT.to_string(),
            "sh".into(),
            "/docs/a.md".into(),
            "/docs/a.pdf".into(),
            "pdf".into(),
        ],
    );
}

#[test]
fn pdf_compress_renders_the_ghostscript_argv() {
    let tool = builtin_registry().get("pdf.compress").unwrap().clone();
    let Capability::Command(spec) = &tool.capability else {
        panic!("expected Command");
    };

    let mut args = BTreeMap::new();
    args.insert("input".to_string(), ArgValue::Path("/docs/a.pdf".into()));
    args.insert("quality".to_string(), ArgValue::Str("printer".into()));
    let out = std::path::PathBuf::from("/docs/a-compressed.pdf");
    assert_eq!(
        render_argv(&spec.args, &args, Some(&out)).unwrap(),
        vec![
            "-sDEVICE=pdfwrite".to_string(),
            "-dCompatibilityLevel=1.4".into(),
            "-dPDFSETTINGS=/printer".into(),
            "-dNOPAUSE".into(),
            "-dBATCH".into(),
            "-sOutputFile=/docs/a-compressed.pdf".into(),
            "/docs/a.pdf".into(),
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
    assert_eq!(
        ids,
        vec![
            "audio.extract",
            "color.pick",
            "document.convert",
            "image.convert",
            "pdf.compress",
            "video.convert"
        ]
    );
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
