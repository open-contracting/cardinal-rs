#![feature(let_chains)]

use std::env;
use std::fs::File;
use std::io::Write;
use std::path::Path;

use glob::glob;

// Rust has no built-in parametrize feature. We can't use snapshot crates, because key order is non-deterministic.
// See https://github.com/la10736/rstest/issues/163
// https://stackoverflow.com/a/49056967/244258
fn main() {
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
        let function = name.to_ascii_lowercase().replace('-', "_");
        let mut parts = name.splitn(3, '-');
        let indicator = parts.next().unwrap();

        let setting = if let Some(field) = parts.next() && let Some(value) = parts.next() {
            format!("indicators::{indicator} {{ {field}: Some({value}), ..Default::default() }}")
        } else {
            "Default::default()".into()
        };

        write!(
            file,
            r#"
#[test]
fn {function}() {{
    check_indicators("indicators/{name}", Settings {{
        {indicator}: Some({setting}),
        ..Default::default()
    }})
}}
"#
        )
        .unwrap();
    }
}
