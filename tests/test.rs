use assert_cmd::assert::Assert;
use assert_cmd::Command;
use predicates::prelude::*;
use rstest::rstest;
#[cfg(unix)]
use tempfile::NamedTempFile;
use trycmd;

fn coverage(args: &[&str]) -> Assert {
    Command::cargo_bin(env!("CARGO_PKG_NAME"))
        .unwrap()
        .arg("coverage")
        .args(args)
        .assert()
}

#[test]
fn documentation() {
    trycmd::TestCases::new().case("README.md").case("docs/**/*.md");
}

#[test]
fn success_stdin() {
    let alt1 = predicate::eq("{\"\": 1, \"[]\": 1}\n");
    let alt2 = predicate::eq("{\"[]\": 1, \"\": 1}\n");
    Command::cargo_bin(env!("CARGO_PKG_NAME"))
        .unwrap()
        .args(&["coverage", "-"])
        .write_stdin("[0]") // coverage/base_array.jsonl
        .assert()
        .success()
        .stdout(alt1.or(alt2));
}

#[test]
fn success_file() {
    let alt1 = predicate::eq("{\"\": 1, \"[]\": 1}\n");
    let alt2 = predicate::eq("{\"[]\": 1, \"\": 1}\n");
    coverage(&["tests/fixtures/coverage/base_array.jsonl"])
        .success()
        .stdout(alt1.or(alt2));
}

#[test]
fn failure_directory() {
    let message = "error: tests: Is a directory, not a file\n";
    coverage(&["tests"])
        .failure()
        .stderr(predicate::str::starts_with(message));
}

#[test]
fn failure_notfound() {
    let pattern =
        r"^error: notfound: (No such file or directory|The system cannot find the file specified\.) \(os error 2\)\n";
    coverage(&["notfound"])
        .failure()
        .stderr(predicate::str::is_match(pattern).unwrap());
}

#[cfg(unix)]
#[test]
fn failure_permissiondenied() {
    use std::os::unix::fs::PermissionsExt;

    let mut tempfile = NamedTempFile::new().unwrap();

    let file = tempfile.as_file_mut();
    let mut permissions = file.metadata().unwrap().permissions();
    permissions.set_mode(0o000);
    file.set_permissions(permissions).unwrap();

    let pattern = r"^error: \S+: Permission denied \(os error 13\)\n";
    coverage(&[tempfile.path().to_str().unwrap()])
        .failure()
        .stderr(predicate::str::is_match(pattern).unwrap());
}

#[test]
fn error_invalid_multiline() {
    let msg1 =
        " WARN  ocdscardinal > Line 1 is invalid JSON, skipping. [EOF while parsing an object at line 1 column 1]\n";
    let msg2 = " WARN  ocdscardinal > Line 2 is invalid JSON, skipping. [expected value at line 1 column 1]\n";
    coverage(&["tests/fixtures/coverage/invalid_multiline.jsonl"])
        .success()
        .stderr(predicate::str::contains(msg1).and(predicate::str::contains(msg2)));
}

#[test]
fn error_invalid_utf8() {
    let msg = " WARN  ocdscardinal > Line 1 caused an I/O error, skipping. [stream did not contain valid UTF-8]\n";
    coverage(&["tests/fixtures/coverage/invalid_utf8.jsonl"])
        .success()
        .stderr(msg);
}

#[rstest]
#[case("invalid_array_quote_first", ":", 1, 5)]
#[case("invalid_array_quote_last", ":", 1, 13)]
#[case("invalid_object_quote_first", ":", 1, 4)]
#[case("invalid_object_quote_last", ":", 2, 4)]
#[case("invalid_brace_first", "EOF", 1, 1)]
#[case("invalid_brace_last", "EOF", 2, 1)]
fn error_invalid_json(#[case] name: &str, #[case] infix: &str, #[case] line: u8, #[case] column: u8) {
    let infix = match infix {
        ":" => "expected `:`",
        "EOF" => "EOF while parsing an object",
        &_ => unreachable!(),
    };

    let msg =
        format!(" WARN  ocdscardinal > Line {line} is invalid JSON, skipping. [{infix} at line 1 column {column}]\n");
    coverage(&[&format!("tests/fixtures/coverage/{name}.jsonl")])
        .success()
        .stderr(msg);
}
