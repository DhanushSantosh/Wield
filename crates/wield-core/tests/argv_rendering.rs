use std::collections::BTreeMap;
use std::path::PathBuf;
use wield_core::args::ArgValue;
use wield_core::descriptor::{ArgValueLiteral, CommandArg, When};
use wield_core::template::{render_argv, render_output_name};

fn map(pairs: &[(&str, ArgValue)]) -> BTreeMap<String, ArgValue> {
    pairs
        .iter()
        .cloned()
        .map(|(key, value)| (key.to_string(), value))
        .collect()
}

#[test]
fn substitutes_input_and_output_and_derived() {
    let effective = map(&[
        ("input", ArgValue::Path(PathBuf::from("/home/a/pic 1.png"))),
        ("format", ArgValue::Str("webp".into())),
    ]);
    let args: Vec<CommandArg> = vec![
        "{input}".into(),
        "-quality".into(),
        "82".into(),
        "{output}".into(),
    ];
    let output = PathBuf::from("/home/a/pic 1.webp");
    let argv = render_argv(&args, &effective, Some(&output)).unwrap();
    assert_eq!(
        argv,
        vec![
            "/home/a/pic 1.png".to_string(),
            "-quality".into(),
            "82".into(),
            "/home/a/pic 1.webp".into(),
        ]
    );
}

#[test]
fn drops_segment_whose_optional_arg_is_absent() {
    let effective = map(&[("input", ArgValue::Path(PathBuf::from("/x/a.png")))]);
    let args: Vec<CommandArg> = vec!["{input}".into(), "-resize".into(), "{width}x".into()];
    let argv = render_argv(&args, &effective, None).unwrap();
    assert_eq!(argv, vec!["/x/a.png".to_string(), "-resize".into()]);
}

#[test]
fn drops_both_paired_segments_via_matching_when() {
    let effective = map(&[("input", ArgValue::Path(PathBuf::from("/x/a.png")))]);
    let gate = Some(When {
        arg: "resize".into(),
        in_values: vec![ArgValueLiteral::Bool(true)],
    });
    let args = vec![
        CommandArg::from("{input}"),
        CommandArg {
            template: "-resize".into(),
            when: gate.clone(),
        },
        CommandArg {
            template: "{width}x".into(),
            when: gate,
        },
    ];
    let argv = render_argv(&args, &effective, None).unwrap();
    assert_eq!(argv, vec!["/x/a.png".to_string()]);
}

#[test]
fn render_output_name_uses_stem_and_format() {
    let effective = map(&[
        ("input", ArgValue::Path(PathBuf::from("/x/holiday.jpeg"))),
        ("format", ArgValue::Str("png".into())),
    ]);
    let name = render_output_name("{input_stem}.{format}", &effective).unwrap();
    assert_eq!(name, "holiday.png");
}

#[test]
fn output_placeholder_without_path_is_an_error() {
    let effective = map(&[("input", ArgValue::Path(PathBuf::from("/x/a.png")))]);
    let args: Vec<CommandArg> = vec!["{output}".into()];
    assert!(render_argv(&args, &effective, None).is_err());
}

#[test]
fn arg_when_set_drops_paired_option_when_absent() {
    let gate = |arg: &str| {
        Some(When {
            arg: arg.into(),
            in_values: vec![],
        })
    };
    let args = vec![
        CommandArg::from("{input}"),
        CommandArg {
            template: "-resize".into(),
            when: gate("width"),
        },
        CommandArg {
            template: "{width}x".into(),
            when: gate("width"),
        },
        CommandArg::from("{output}"),
    ];
    let out = PathBuf::from("/x/a.png");

    let absent = map(&[("input", ArgValue::Path("/x/a.png".into()))]);
    assert_eq!(
        render_argv(&args, &absent, Some(&out)).unwrap(),
        vec!["/x/a.png".to_string(), "/x/a.png".to_string()],
    );

    let present = map(&[
        ("input", ArgValue::Path("/x/a.png".into())),
        ("width", ArgValue::Int(640)),
    ]);
    assert_eq!(
        render_argv(&args, &present, Some(&out)).unwrap(),
        vec![
            "/x/a.png".to_string(),
            "-resize".into(),
            "640x".into(),
            "/x/a.png".into()
        ],
    );
}
