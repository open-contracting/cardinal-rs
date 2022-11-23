use std::env;
use std::fs::File;
use std::io::Write;
use std::path::Path;

use glob::glob;

// Rust has no built-in parametrize feature. We can't use snapshot crates, because key order is non-deterministic.
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
    check("{name}")
}}
"#,
            name = name,
        )
        .unwrap();
    }

    let path = Path::new(&env::var("OUT_DIR").unwrap()).join("test.include");
    let mut file = File::create(path).unwrap();

    let params = [
        ("invalid_array_quote_first", ":", 1, 5),
        ("invalid_array_quote_last", ":", 1, 13),
        ("invalid_object_quote_first", ":", 1, 4),
        ("invalid_object_quote_last", ":", 2, 4),
        ("invalid_brace_first", "EOF", 1, 1),
        ("invalid_brace_last", "EOF", 2, 1),
    ];

    for (name, infix, line, column) in params {
        write!(
            file,
            r#"
#[test]
fn error_{name}() {{
    check("{name}", "{infix}", {line}, {column})
}}
"#,
            name = name,
            infix = infix,
            line = line,
            column = column,
        )
        .unwrap();
    }
}
