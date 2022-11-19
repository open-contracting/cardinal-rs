use assert_cmd::Command;
use assert_cmd::assert::Assert;

fn coverage(args: &[&str]) -> Assert {
    Command::cargo_bin(env!("CARGO_PKG_NAME")).unwrap().args(&["coverage", "--threads", "1"]).args(args).assert()
}

#[test]
fn test_success() {
    coverage(&["tests/fixtures/base_object.jsonl"]).success().stdout("{\"a\": 1}\n");
}


#[test]
fn test_error_invalid_multiline() {
    coverage(&["tests/fixtures/invalid_multiline.jsonl"]).success().stderr(
        " WARN  libocdscardinal > Line 1 is invalid JSON, skipping. [EOF while parsing an object at line 1 column 1]\
       \n WARN  libocdscardinal > Line 2 is invalid JSON, skipping. [expected value at line 1 column 1]\n"
    );
}

#[test]
fn test_error_invalid_utf8() {
    coverage(&["tests/fixtures/invalid_utf8.jsonl"]).success().stderr(
        " WARN  libocdscardinal > Line 1 caused an I/O error, skipping. [stream did not contain valid UTF-8]\n"
    );
}

fn check(name: &str, infix: &str, line: u8, column: u8) {
    let infix = match infix {
        ":" => "expected `:`",
        "EOF" => "EOF while parsing an object",
        &_ => todo!(),
    };

    coverage(&[&format!("tests/fixtures/{name}.jsonl")]).success().stderr(
        format!(" WARN  libocdscardinal > Line {line} is invalid JSON, skipping. [{infix} at line 1 column {column}]\n")
    );
}

include!(concat!(env!("OUT_DIR"), "/test.include"));
