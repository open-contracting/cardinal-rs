use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;

use anyhow::{Context, Result};
use log::warn;
use rayon::prelude::*;
use serde_json::Value;

#[derive(Debug)]
pub struct Coverage {
    counts: HashMap<String, u32>,
}

impl Coverage {
    fn new() -> Self {
        Coverage {
            counts: HashMap::new(),
        }
    }

    pub fn counts(&self) -> &HashMap<String, u32> {
        &self.counts
    }

    pub fn run(filename: PathBuf) -> Result<Coverage> {
        let file = File::open(&filename)
            .with_context(|| format!("Failed to read '{}'", filename.display()))?;

        let coverage = BufReader::new(file)
            .lines()
            .enumerate()
            .par_bridge()
            .fold(Coverage::new, |mut coverage, (i, result)| {
                match result {
                    Ok(string) => {
                        match serde_json::from_str(&string) {
                            Ok(value) => {
                                coverage.add(value, &mut Vec::with_capacity(16));
                            }
                            Err(e) => {
                                // Skip empty lines silently.
                                // https://stackoverflow.com/a/64361042/244258
                                if !string.as_bytes().iter().all(u8::is_ascii_whitespace) {
                                    warn!("Line {} is invalid JSON, skipping. [{e}]", i + 1)
                                }
                            }
                        }
                    }
                    // Err: https://doc.rust-lang.org/std/io/enum.ErrorKind.html
                    // https://github.com/rust-lang/rust/blob/1.65.0/library/std/src/io/buffered/bufreader.rs#L362-L365
                    Err(e) => warn!("Line {} caused an I/O error, skipping. [{e}]", i + 1),
                }

                coverage
            })
            .reduce(Coverage::new, |mut coverage, other| {
                for (k, v) in other.counts {
                    coverage.increment(k, v);
                }

                coverage
            });

        Ok(coverage)
    }

    // The longest path has 6 parts (as below or contracts/implementation/transactions/payer/identifier/id).
    // The longest pointer has 10 parts (contracts/0/amendments/0/unstructuredChanges/0/oldValue/classifications/0/id).
    fn add(&mut self, value: Value, path: &mut Vec<String>) {
        // Note:
        // - Within an object, a "" key and the object itself share the same path.
        // - Within an array, literal values, non-empty objects and non-empty arrays share the same path.
        // - At the top-level, literal values, non-empty objects and non-empty arrays share the same path.
        // - If a member is repeated, the last is measured.
        //
        // Using a String as the key with `join("/")` is faster than Vec<String> as the key with `to_vec()`.
        match value {
            Value::Null => {}
            Value::Array(vec) => {
                if !vec.is_empty() {
                    self.increment(path.join("/"), 1);
                }
                path.push(String::from("-"));
                for i in vec {
                    self.add(i, path);
                }
                path.pop();
            }
            Value::Object(map) => {
                if !map.is_empty() {
                    self.increment(path.join("/"), 1);
                }
                for (k, v) in map {
                    path.push(k);
                    self.add(v, path);
                    path.pop();
                }
            }
            Value::String(string) => {
                if !string.is_empty() {
                    self.increment(path.join("/"), 1);
                }
            }
            // number, boolean
            _ => {
                self.increment(path.join("/"), 1);
            }
        }
    }

    fn increment(&mut self, path: String, delta: u32) {
        self.counts
            .entry(path)
            .and_modify(|count| *count += delta)
            .or_insert(delta);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::io::ErrorKind;
    use std::path::Path;

    use pretty_assertions::assert_eq;
    use regex::Regex;
    use tempfile::NamedTempFile;

    #[test]
    fn test_notfound() {
        let result = Coverage::run(PathBuf::from("notfound"));
        let error = result.unwrap_err();
        let message = format!("{:#}", error);
        // macos-latest, ubuntu-latest: "No such file or directory"
        // windows-latest: "The system cannot find the file specified."
        let re = Regex::new(r"Failed to read '\S+': [A-Za-z. ]+ \(os error 2\)").unwrap();

        assert!(re.is_match(&message), "Error did not match '{message}'");
        assert_eq!(
            error.downcast::<std::io::Error>().unwrap().kind(),
            ErrorKind::NotFound
        );
    }

    #[cfg(unix)]
    #[test]
    fn test_permissiondenied() {
        use std::os::unix::fs::PermissionsExt;

        let mut tempfile = NamedTempFile::new().unwrap();

        let file = tempfile.as_file_mut();
        let mut permissions = file.metadata().unwrap().permissions();
        permissions.set_mode(0o000);
        file.set_permissions(permissions).unwrap();

        let result = Coverage::run(tempfile.path().to_path_buf());
        let error = result.unwrap_err();
        let message = format!("{:#}", error);
        let re = Regex::new(r"Failed to read '\S+': Permission denied \(os error 13\)").unwrap();

        assert!(re.is_match(&message), "Error did not match '{message}'");
        assert_eq!(
            error.downcast::<std::io::Error>().unwrap().kind(),
            ErrorKind::PermissionDenied
        );
    }

    fn check(name: &str) {
        let fixtures = Path::new("tests/fixtures/coverage");

        let inpath = fixtures.join(format!("{name}.jsonl"));
        let result = Coverage::run(inpath);

        let outpath = fixtures.join(format!("{name}.expected"));
        let file = File::open(outpath).unwrap();
        let reader = BufReader::new(file);
        let expected = serde_json::from_reader(reader).unwrap();

        assert_eq!(result.unwrap().counts, expected);
    }

    include!(concat!(env!("OUT_DIR"), "/lib.include"));
}
