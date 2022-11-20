use assert_cmd::assert::Assert;
use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::NamedTempFile;

fn coverage(args: &[&str]) -> Assert {
    Command::cargo_bin(env!("CARGO_PKG_NAME"))
        .unwrap()
        .arg("coverage")
        .args(args)
        .assert()
}

#[test]
fn test_success() {
    let alt1 = predicate::eq("{\"\": 1, \"[]\": 1}\n");
    let alt2 = predicate::eq("{\"[]\": 1, \"\": 1}\n");
    coverage(&["tests/fixtures/coverage/base_array.jsonl"])
        .success()
        .stdout(alt1.or(alt2));
}

#[test]
fn test_failure_directory() {
    let message = "error: tests: Is a directory, not a file\n";
    coverage(&["tests"])
        .failure()
        .stderr(predicate::str::contains(message));
}

#[test]
fn test_failure_notfound() {
    let pattern = r"^error: notfound: (No such file or directory|The system cannot find the file specified\.) \(os error 2\)\n";
    coverage(&["notfound"])
        .failure()
        .stderr(predicate::str::is_match(pattern).unwrap());
}

#[cfg(unix)]
#[test]
fn test_failure_permissiondenied() {
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
fn test_error_invalid_multiline() {
    let msg1 = " WARN  libocdscardinal > Line 1 is invalid JSON, skipping. [EOF while parsing an object at line 1 column 1]\n";
    let msg2 = " WARN  libocdscardinal > Line 2 is invalid JSON, skipping. [expected value at line 1 column 1]\n";
    coverage(&["tests/fixtures/coverage/invalid_multiline.jsonl"])
        .success()
        .stderr(predicate::str::contains(msg1).and(predicate::str::contains(msg2)));
}

#[test]
fn test_error_invalid_utf8() {
    let msg = " WARN  libocdscardinal > Line 1 caused an I/O error, skipping. [stream did not contain valid UTF-8]\n";
    coverage(&["tests/fixtures/coverage/invalid_utf8.jsonl"])
        .success()
        .stderr(msg);
}

fn check(name: &str, infix: &str, line: u8, column: u8) {
    let infix = match infix {
        ":" => "expected `:`",
        "EOF" => "EOF while parsing an object",
        &_ => todo!(),
    };

    let msg = format!(" WARN  libocdscardinal > Line {line} is invalid JSON, skipping. [{infix} at line 1 column {column}]\n");
    coverage(&[&format!("tests/fixtures/coverage/{name}.jsonl")])
        .success()
        .stderr(msg);
}

include!(concat!(env!("OUT_DIR"), "/test.include"));
