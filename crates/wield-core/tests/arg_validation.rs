use std::collections::BTreeMap;
use wield_core::args::{visible_args, ArgValue};
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
