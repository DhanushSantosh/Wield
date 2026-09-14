//! `wield-tools` — the built-in Wield tool descriptors.
//!
//! Each built-in is a [`wield_core::Descriptor`] constructed with the typed
//! builders and validated on build. [`builtin_registry`] is the single source
//! every surface (palette, tray, CLI) reads from.

pub mod audio_extract;
pub mod color_pick;
pub mod document_convert;
pub mod image_convert;
pub mod pdf_compress;
pub mod video_convert;

use wield_core::Registry;

/// The registry of every built-in tool, validated. Panics only if a built-in
/// descriptor is malformed — a programming error the snapshot test catches.
pub fn builtin_registry() -> Registry {
    let mut registry = Registry::new();
    registry
        .register(audio_extract::descriptor())
        .expect("audio.extract registers");
    registry
        .register(color_pick::descriptor())
        .expect("color.pick registers");
    registry
        .register(document_convert::descriptor())
        .expect("document.convert registers");
    registry
        .register(image_convert::descriptor())
        .expect("image.convert registers");
    registry
        .register(pdf_compress::descriptor())
        .expect("pdf.compress registers");
    registry
        .register(video_convert::descriptor())
        .expect("video.convert registers");
    registry
}
