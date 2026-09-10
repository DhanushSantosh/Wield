//! The `image.convert` built-in — format / resize / recompress via ImageMagick.

use std::time::Duration;
use wield_core::{
    ArgSpecBuilder, ArgType, ArgValueLiteral, Category, CommandSpecBuilder, Descriptor,
    DescriptorBuilder, FileFilter, OutputDir, OutputSpec, Requires,
};

const IMAGE_EXTS: &[&str] = &["png", "jpg", "jpeg", "webp", "gif", "bmp", "tiff", "avif"];
const FORMATS: &[&str] = &["png", "jpg", "webp", "gif", "bmp", "tiff", "avif"];

/// Descriptor for `image.convert`. Runs `magick <input> [-resize Wx] [-quality N]
/// <output>`; the output format follows the `{format}` extension of the output
/// filename. `width` and `quality` are optional — absent options drop from argv.
pub fn descriptor() -> Descriptor {
    DescriptorBuilder::new("image.convert", "Convert image", Category::Convert)
        .keywords(&[
            "image", "convert", "resize", "compress", "png", "jpeg", "webp", "format",
        ])
        .arg(
            ArgSpecBuilder::new(
                "input",
                "Image",
                ArgType::File {
                    filters: vec![FileFilter {
                        label: "Images".into(),
                        extensions: IMAGE_EXTS.iter().map(|ext| (*ext).to_owned()).collect(),
                    }],
                    multiple: false,
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
            .default(ArgValueLiteral::Str("png".into()))
            .build(),
        )
        .arg(
            ArgSpecBuilder::new(
                "width",
                "Width (px)",
                ArgType::Int {
                    range: Some([1, 20_000]),
                    step: Some(1),
                },
            )
            .help("Resize to this width, keeping aspect ratio. Leave blank to keep the original size.")
            .build(),
        )
        .arg(
            ArgSpecBuilder::new(
                "quality",
                "Quality",
                ArgType::Int {
                    range: Some([1, 100]),
                    step: Some(1),
                },
            )
            .help(
                "Compression quality for lossy formats (JPEG, WebP, AVIF). Leave blank for the format default.",
            )
            .build(),
        )
        .requires(Requires::Binary("magick".into()))
        .output(OutputSpec::File {
            name: "{input_stem}.{format}".into(),
            dir: OutputDir::SameAsInput,
        })
        .command(
            CommandSpecBuilder::new("magick")
                .arg("{input}")
                .arg_when_set("-resize", "width")
                .arg_when_set("{width}x", "width")
                .arg_when_set("-quality", "quality")
                .arg_when_set("{quality}", "quality")
                .arg("{output}")
                .timeout(Duration::from_secs(120)),
        )
        .build()
        .expect("image.convert descriptor is valid")
}
