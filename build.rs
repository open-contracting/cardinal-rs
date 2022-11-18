use std::env;
use std::fs::File;
use std::io::Write;
use std::path::Path;

use glob::glob;

// Rust has no built-in parametrize feature.
// https://stackoverflow.com/a/49056967/244258
fn main() {
    let path = Path::new(&env::var("OUT_DIR").unwrap()).join("tests.rs");
    let mut file = File::create(path).unwrap();

    for entry in glob("tests/fixtures/*.jsonl").expect("Failed to read glob pattern") {
        let path = entry.unwrap();
        let input = path.file_name().unwrap().to_str().unwrap();
        let stem = path.file_stem().unwrap().to_str().unwrap();
        let output = format!("{stem}.expected");

        write!(file, r#"
#[test]
fn test_{name}() {{
    check("{input}", "{output}")
}}
"#, name=stem, input=input, output=output).unwrap()
    }
}
