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
fn {name}() {{
    check_coverage("coverage/{name}")
}}
"#
        )
        .unwrap();
    }

    for entry in glob("tests/fixtures/indicators/*.jsonl").expect("Failed to read glob pattern") {
        let path = entry.unwrap();
        let name = path.file_stem().unwrap().to_str().unwrap();
        let function = name.to_ascii_lowercase();
        let field = name.split('_').next().unwrap();

        write!(
            file,
            r#"
#[test]
fn {function}() {{
    check_indicators("indicators/{name}", Settings {{
        {field}: Some(Default::default()),
        ..Default::default()
    }})
}}
"#
        )
        .unwrap();
    }
}
