#![feature(let_chains)]

use std::cmp::Ordering;
use std::collections::VecDeque;
use std::env;
use std::fs::File;
use std::io::Write;
use std::path::Path;

use glob::glob;
use itertools::Itertools;

// Rust has no built-in parametrize feature. We can't use snapshot crates, because key order is non-deterministic.
// See https://github.com/la10736/rstest/issues/163
// https://stackoverflow.com/a/49056967/244258
fn main() {
    // https://pyo3.rs/v0.25.1/building-and-distribution.html#macos
    pyo3_build_config::add_extension_module_link_args();

    let path = Path::new(&env::var("OUT_DIR").unwrap()).join("lib.include");
    let mut file = File::create(path).unwrap();

    for entry in glob("tests/fixtures/coverage/*.jsonl").expect("Failed to read glob pattern") {
        let path = entry.unwrap();
        let name = path.file_stem().unwrap().to_str().unwrap();

        write!(
            file,
            r#"
#[test]
fn coverage_{name}() {{
    check_coverage("coverage/{name}")
}}
"#
        )
        .unwrap();
    }

    for entry in glob("tests/fixtures/prepare/*.jsonl").expect("Failed to read glob pattern") {
        let path = entry.unwrap();
        let name = path.file_stem().unwrap().to_str().unwrap();

        write!(
            file,
            r#"
#[test]
fn prepare_{name}() {{
    check_prepare("prepare/{name}")
}}
"#
        )
        .unwrap();
    }

    for entry in glob("tests/fixtures/indicators/*.jsonl").expect("Failed to read glob pattern") {
        let path = entry.unwrap();
        let name = path.file_stem().unwrap().to_str().unwrap();
        let function = name.to_ascii_lowercase().replace(['-', '+'], "_");
        let parts = name.replace('+', "|");
        let mut parts = parts.split('-').collect::<VecDeque<_>>();
        let ident = parts.pop_front().unwrap();

        let setting = match parts.len().cmp(&2) {
            Ordering::Less => "Default::default()".into(),
            Ordering::Equal => {
                let field = parts[0];
                let value = parts[1];
                if value.as_bytes().iter().all(u8::is_ascii_digit) {
                    format!("indicators::{ident} {{ {field}: Some({value}), ..Default::default() }}")
                } else {
                    format!("indicators::{ident} {{ {field}: Some(String::from(\"{value}\")), ..Default::default() }}")
                }
            }
            Ordering::Greater => {
                let field = parts.pop_front().unwrap();
                let tuples = parts
                    .iter()
                    .tuples()
                    .map(|(key, value)| format!("(String::from(\"{key}\"), {value})"))
                    .join(",");
                format!("indicators::{ident} {{ {field}: Some(HashMap::from([{tuples}])), ..Default::default() }}")
            }
        };

        write!(
            file,
            r#"
#[test]
fn {function}() {{
    check_indicators("indicators/{name}", Settings {{
        {ident}: Some({setting}),
        no_price_comparison_procurement_methods: Some(String::from("NPC")),
        exclusions: Some(Exclusions {{ procurement_method_details: Some(String::from("EXC")) }}),
        ..Default::default()
    }})
}}
"#
        )
        .unwrap();
    }
}
