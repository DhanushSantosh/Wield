//! The `video.convert` built-in — format / resolution / quality via ffmpeg.

use std::time::Duration;
use wield_core::{
    ArgSpecBuilder, ArgType, ArgValueLiteral, Category, CommandSpecBuilder, Descriptor,
    DescriptorBuilder, FileFilter, OutputDir, OutputSpec, ProgressSpec, Requires,
};

const VIDEO_EXTS: &[&str] = &["mp4", "mov", "mkv", "webm", "avi"];
const FORMATS: &[&str] = &["mp4", "webm", "mkv"];
const RESOLUTIONS: &[&str] = &["original", "1080p", "720p", "480p"];

/// Descriptor for `video.convert`. Runs `ffmpeg -y -i <input> [-vf
/// scale=-2:H] [-crf N] -progress pipe:2 -nostats <output>`. `resolution`
/// and `quality` are optional - absent options drop from argv, matching
/// `image.convert`'s established pattern. `input` accepts multiple files
/// (`multiple: true`) - `wield-core`'s executor converts each one through
/// the same single-file path and reports a summary; see
/// `docs/superpowers/specs/2026-09-14-wield-m2a-video-convert-design.md`.
pub fn descriptor() -> Descriptor {
    DescriptorBuilder::new("video.convert", "Convert video", Category::Convert)
        .keywords(&[
            "video", "convert", "ffmpeg", "mp4", "webm", "mkv", "resolution", "format",
        ])
        .arg(
            ArgSpecBuilder::new(
                "input",
                "Video",
                ArgType::File {
                    filters: vec![FileFilter {
                        label: "Videos".into(),
                        extensions: VIDEO_EXTS.iter().map(|ext| (*ext).to_owned()).collect(),
                    }],
                    multiple: true,
                },
            )
            .required(true)
            .build(),
        )
        .arg(
            ArgSpecBuilder::new(
                "format",
                "Output format",
                ArgType::Enum {
                    options: FORMATS.iter().map(|fmt| (*fmt).to_owned()).collect(),
                },
            )
            .required(true)
            .default(ArgValueLiteral::Str("mp4".into()))
            .build(),
        )
        .arg(
            ArgSpecBuilder::new(
                "resolution",
                "Resolution",
                ArgType::Enum {
                    options: RESOLUTIONS.iter().map(|r| (*r).to_owned()).collect(),
                },
            )
            .required(true)
            .default(ArgValueLiteral::Str("original".into()))
            .help("\"original\" keeps the source resolution unchanged.")
            .build(),
        )
        .arg(
            ArgSpecBuilder::new(
                "quality",
                "Quality (CRF)",
                ArgType::Int {
                    range: Some([0, 51]),
                    step: Some(1),
                },
            )
            .help(
                "Lower is higher quality (libx264's CRF scale - the opposite of most \"quality\" sliders). Leave blank for ffmpeg's own default.",
            )
            .build(),
        )
        .requires(Requires::Binary("ffmpeg".into()))
        .output(OutputSpec::File {
            name: "{input_stem}.{format}".into(),
            dir: OutputDir::SameAsInput,
        })
        .command(
            CommandSpecBuilder::new("ffmpeg")
                .arg("-y")
                .arg("-i")
                .arg("{input}")
                .arg_when("-vf", "resolution", vec![ArgValueLiteral::Str("1080p".into())])
                .arg_when(
                    "scale=-2:1080",
                    "resolution",
                    vec![ArgValueLiteral::Str("1080p".into())],
                )
                .arg_when("-vf", "resolution", vec![ArgValueLiteral::Str("720p".into())])
                .arg_when(
                    "scale=-2:720",
                    "resolution",
                    vec![ArgValueLiteral::Str("720p".into())],
                )
                .arg_when("-vf", "resolution", vec![ArgValueLiteral::Str("480p".into())])
                .arg_when(
                    "scale=-2:480",
                    "resolution",
                    vec![ArgValueLiteral::Str("480p".into())],
                )
                .arg_when_set("-crf", "quality")
                .arg_when_set("{quality}", "quality")
                .arg("-progress")
                .arg("pipe:2")
                .arg("-nostats")
                .arg("{output}")
                .progress(ProgressSpec::FfmpegDuration)
                .timeout(Duration::from_secs(1800)),
        )
        .build()
        .expect("video.convert descriptor is valid")
}
