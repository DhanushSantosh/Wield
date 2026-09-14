//! The `audio.extract` built-in — extract or convert an audio track via ffmpeg.

use std::time::Duration;
use wield_core::{
    ArgSpecBuilder, ArgType, ArgValueLiteral, Category, CommandSpecBuilder, Descriptor,
    DescriptorBuilder, FileFilter, OutputDir, OutputSpec, ProgressSpec, Requires,
};

const INPUT_EXTS: &[&str] = &[
    "mp4", "mov", "mkv", "webm", "avi", "mp3", "wav", "flac", "m4a", "aac",
];
const FORMATS: &[&str] = &["mp3", "m4a", "flac", "wav"];
const BITRATES: &[&str] = &["128k", "192k", "256k", "320k"];

/// Descriptor for `audio.extract`. Runs `ffmpeg -y -i <input> -vn
/// -codec:a <codec> [-b:a <bitrate>] -progress pipe:2 -nostats <output>`.
/// `codec` is picked per `format` via `arg_when` (libmp3lame/aac/flac/
/// pcm_s16le) - `format`'s enum value is the output *extension* (it feeds
/// `{format}` in the output name template, same mechanism
/// `video.convert` uses), which is why the m4a option's codec is `aac`
/// (two different strings, kept deliberately separate - a literal `.aac`
/// extension would select ffmpeg's ADTS muxer instead of the more
/// compatible, more precise MP4-family `.m4a` container; verified live,
/// see the design spec §3-4). `quality` (a bitrate preset) only applies
/// to the lossy formats (mp3/m4a) and is hidden for flac/wav via its own
/// `when` gate - the same mechanism `video.convert`'s resolution-gated
/// `-vf`/`-crf` segments use. `-vn` is a safe no-op on an audio-only
/// input (verified live), so this one descriptor covers both "extract
/// audio from a video" and "convert between audio formats". See
/// `docs/superpowers/specs/2026-09-14-wield-m2b-audio-extract-design.md`.
pub fn descriptor() -> Descriptor {
    DescriptorBuilder::new("audio.extract", "Extract audio", Category::Convert)
        .keywords(&[
            "audio", "extract", "convert", "ffmpeg", "mp3", "m4a", "flac", "wav", "track",
        ])
        .arg(
            ArgSpecBuilder::new(
                "input",
                "Video or audio",
                ArgType::File {
                    filters: vec![FileFilter {
                        label: "Video & audio".into(),
                        extensions: INPUT_EXTS.iter().map(|ext| (*ext).to_owned()).collect(),
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
            .default(ArgValueLiteral::Str("mp3".into()))
            .build(),
        )
        .arg(
            ArgSpecBuilder::new(
                "quality",
                "Bitrate",
                ArgType::Enum {
                    options: BITRATES.iter().map(|b| (*b).to_owned()).collect(),
                },
            )
            .when(
                "format",
                vec![
                    ArgValueLiteral::Str("mp3".into()),
                    ArgValueLiteral::Str("m4a".into()),
                ],
            )
            .help("Only applies to mp3/m4a - flac and wav are lossless.")
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
                .arg("-vn")
                .arg_when(
                    "-codec:a",
                    "format",
                    vec![ArgValueLiteral::Str("mp3".into())],
                )
                .arg_when(
                    "libmp3lame",
                    "format",
                    vec![ArgValueLiteral::Str("mp3".into())],
                )
                .arg_when(
                    "-codec:a",
                    "format",
                    vec![ArgValueLiteral::Str("m4a".into())],
                )
                .arg_when("aac", "format", vec![ArgValueLiteral::Str("m4a".into())])
                .arg_when(
                    "-codec:a",
                    "format",
                    vec![ArgValueLiteral::Str("flac".into())],
                )
                .arg_when("flac", "format", vec![ArgValueLiteral::Str("flac".into())])
                .arg_when(
                    "-codec:a",
                    "format",
                    vec![ArgValueLiteral::Str("wav".into())],
                )
                .arg_when(
                    "pcm_s16le",
                    "format",
                    vec![ArgValueLiteral::Str("wav".into())],
                )
                .arg_when_set("-b:a", "quality")
                .arg_when_set("{quality}", "quality")
                .arg("-progress")
                .arg("pipe:2")
                .arg("-nostats")
                .arg("{output}")
                .progress(ProgressSpec::FfmpegDuration)
                .timeout(Duration::from_secs(300)),
        )
        .build()
        .expect("audio.extract descriptor is valid")
}
