use std::time::Duration;
use wield_core::descriptor::*;
use wield_core::executor::AvailabilityView;
use wield_core::registry::{Registry, RegistryError};

fn cmd_tool(id: &str, binary: &str, keywords: &[&str]) -> Descriptor {
    Descriptor {
        id: ToolId::parse(id).unwrap(),
        title: id.into(),
        keywords: keywords.iter().map(|s| s.to_string()).collect(),
        category: Category::Convert,
        args: vec![ArgSpec {
            name: "input".into(),
            label: "in".into(),
            help: None,
            arg_type: ArgType::File {
                filters: vec![],
                multiple: false,
            },
            default: None,
            required: true,
            when: None,
        }],
        requires: Requires::Binary(binary.into()),
        output: OutputSpec::File {
            name: "{input_stem}.o".into(),
            dir: OutputDir::SameAsInput,
        },
        capability: Capability::Command(CommandSpec {
            binary: binary.into(),
            args: vec!["{input}".into(), "{output}".into()],
            progress: ProgressSpec::None,
            timeout: Duration::from_secs(5),
            success: SuccessSpec::ExitZero,
            combine_inputs: false,
        }),
    }
}

#[test]
fn rejects_duplicate_ids() {
    let mut r = Registry::new();
    r.register(cmd_tool("image.convert", "magick", &[]))
        .unwrap();
    let err = r
        .register(cmd_tool("image.convert", "magick", &[]))
        .unwrap_err();
    assert!(matches!(err, RegistryError::DuplicateId(id) if id == "image.convert"));
}

#[test]
fn rejects_invalid_descriptor() {
    let mut r = Registry::new();
    let mut bad = cmd_tool("image.convert", "magick", &[]);
    if let Capability::Command(ref mut c) = bad.capability {
        c.args.push("{bogus}".into());
    }
    let err = r.register(bad).unwrap_err();
    assert!(matches!(err, RegistryError::Invalid { .. }));
}

#[test]
fn fuzzy_search_ranks_by_relevance() {
    let mut r = Registry::new();
    r.register(cmd_tool("image.convert", "magick", &["picture", "resize"]))
        .unwrap();
    r.register(cmd_tool("video.convert", "ffmpeg", &["movie"]))
        .unwrap();
    let hits = r.search("img conv");
    assert_eq!(hits.first().unwrap().id.as_ref(), "image.convert");
    assert!(r.search("zzz nonsense").is_empty());
}

#[test]
fn blank_query_returns_all_in_order() {
    let mut r = Registry::new();
    r.register(cmd_tool("b.two", "x", &[])).unwrap();
    r.register(cmd_tool("a.one", "y", &[])).unwrap();
    let ids: Vec<_> = r
        .search("  ")
        .iter()
        .map(|d| d.id.as_ref().to_string())
        .collect();
    assert_eq!(ids, vec!["b.two", "a.one"]);
}

#[test]
fn available_filters_on_binaries() {
    let mut r = Registry::new();
    r.register(cmd_tool("image.convert", "magick", &[]))
        .unwrap();
    r.register(cmd_tool("video.convert", "ffmpeg", &[]))
        .unwrap();
    let mut view = AvailabilityView {
        binaries: Default::default(),
        portals: Default::default(),
    };
    view.binaries.insert("magick".into());
    let avail: Vec<_> = r
        .available(&view)
        .iter()
        .map(|d| d.id.as_ref().to_string())
        .collect();
    assert_eq!(avail, vec!["image.convert"]);
}

#[test]
fn snapshot_is_deterministic() {
    let mut a = Registry::new();
    a.register(cmd_tool("b.two", "x", &[])).unwrap();
    a.register(cmd_tool("a.one", "y", &[])).unwrap();
    let mut b = Registry::new();
    b.register(cmd_tool("a.one", "y", &[])).unwrap();
    b.register(cmd_tool("b.two", "x", &[])).unwrap();
    assert_eq!(a.snapshot(), b.snapshot());
}
