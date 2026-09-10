use std::collections::BTreeMap;
use wield_core::args::{validate_args, visible_args, ArgValue};
use wield_core::descriptor::*;

fn specs() -> Vec<ArgSpec> {
    vec![
        ArgSpec {
            name: "resize".into(),
            label: "Resize".into(),
            help: None,
            arg_type: ArgType::Bool,
            default: Some(ArgValueLiteral::Bool(false)),
            required: false,
            when: None,
        },
        ArgSpec {
            name: "width".into(),
            label: "Width".into(),
            help: None,
            arg_type: ArgType::Int {
                range: Some([1, 9999]),
                step: None,
            },
            default: None,
            required: false,
            when: Some(When {
                arg: "resize".into(),
                in_values: vec![ArgValueLiteral::Bool(true)],
            }),
        },
    ]
}

#[test]
fn width_hidden_when_resize_false() {
    let mut values: BTreeMap<String, ArgValue> = BTreeMap::new();
    values.insert("resize".into(), ArgValue::Bool(false));
    let specs = specs();
    let visible = visible_args(&specs, &values);
    assert_eq!(
        visible
            .iter()
            .map(|spec| spec.name.as_str())
            .collect::<Vec<_>>(),
        vec!["resize"]
    );
}

#[test]
fn width_visible_when_resize_true() {
    let mut values: BTreeMap<String, ArgValue> = BTreeMap::new();
    values.insert("resize".into(), ArgValue::Bool(true));
    let specs = specs();
    let visible = visible_args(&specs, &values);
    assert_eq!(visible.len(), 2);
}

#[test]
fn fills_defaults_and_drops_hidden() {
    let mut input = BTreeMap::new();
    input.insert("resize".into(), ArgValue::Bool(false));
    input.insert("width".into(), ArgValue::Int(800));
    let specs = specs();
    let effective = validate_args(&specs, &input).unwrap();
    assert_eq!(effective.get("resize"), Some(&ArgValue::Bool(false)));
    assert!(!effective.contains_key("width"));
}

#[test]
fn rejects_out_of_range_int() {
    let mut input = BTreeMap::new();
    input.insert("resize".into(), ArgValue::Bool(true));
    input.insert("width".into(), ArgValue::Int(99999));
    let specs = specs();
    let errors = validate_args(&specs, &input).unwrap_err();
    assert_eq!(errors[0].field, "width");
}

#[test]
fn rejects_missing_required() {
    let specs = vec![ArgSpec {
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
    }];
    let errors = validate_args(&specs, &BTreeMap::new()).unwrap_err();
    assert_eq!(errors[0].field, "input");
}

#[test]
fn rejects_enum_value_not_in_options() {
    let specs = vec![ArgSpec {
        name: "format".into(),
        label: "f".into(),
        help: None,
        arg_type: ArgType::Enum {
            options: vec!["png".into(), "webp".into()],
        },
        default: None,
        required: true,
        when: None,
    }];
    let mut input = BTreeMap::new();
    input.insert("format".into(), ArgValue::Str("gif".into()));
    let errors = validate_args(&specs, &input).unwrap_err();
    assert_eq!(errors[0].field, "format");
}

#[test]
fn when_set_hides_arg_until_its_dependency_is_present() {
    let specs = vec![
        ArgSpec {
            name: "width".into(),
            label: "w".into(),
            help: None,
            arg_type: ArgType::Int {
                range: None,
                step: None,
            },
            default: None,
            required: false,
            when: None,
        },
        ArgSpec {
            name: "keep_ratio".into(),
            label: "k".into(),
            help: None,
            arg_type: ArgType::Bool,
            default: None,
            required: false,
            when: Some(When {
                arg: "width".into(),
                in_values: vec![],
            }),
        },
    ];
    let mut values = BTreeMap::new();
    assert_eq!(visible_args(&specs, &values).len(), 1);
    values.insert("width".into(), ArgValue::Int(800));
    assert_eq!(visible_args(&specs, &values).len(), 2);
}
