use std::collections::HashMap;
use std::io::BufRead;

use anyhow::Result;
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

    pub fn run(buffer: impl BufRead + Send) -> Result<Coverage> {
        Ok(buffer
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
                                    warn!("Line {} is invalid JSON, skipping. [{e}]", i + 1);
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
            }))
    }

    // The longest path has 6 parts (as below or contracts/implementation/transactions/payer/identifier/id).
    // The longest pointer has 10 parts (contracts/0/amendments/0/unstructuredChanges/0/oldValue/classifications/0/id).
    fn add(&mut self, value: Value, path: &mut Vec<String>) -> bool {
        let mut increment = false;

        // Using a String as the key with `join("/")` is faster than Vec<String> as the key with `to_vec()`.
        match value {
            Value::Null => {}
            Value::Array(vec) => {
                if !vec.is_empty() {
                    path.push(String::from("[]"));
                    for i in vec {
                        increment |= self.add(i, path);
                    }
                    path.pop();
                }
            }
            Value::Object(map) => {
                if !map.is_empty() {
                    path.push(String::from("/"));
                    for (k, v) in map {
                        path.push(k);
                        increment |= self.add(v, path);
                        path.pop();
                    }
                    if increment {
                        self.increment(path.join(""), 1);
                    }
                    path.pop();
                }
            }
            Value::String(string) => {
                increment = !string.is_empty();
            }
            // number, boolean
            _ => {
                increment = true;
            }
        }

        if increment {
            self.increment(path.join(""), 1);
        }
        increment
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

    use std::fs::File;
    use std::io::BufReader;

    use pretty_assertions::assert_eq;

    fn reader(stem: &str, extension: &str) -> BufReader<File> {
        let path = format!("tests/fixtures/coverage/{stem}.{extension}");
        let file = File::open(path).unwrap();

        BufReader::new(file)
    }

    fn check(name: &str) {
        let result = Coverage::run(reader(name, "jsonl"));
        let expected = serde_json::from_reader(reader(name, "expected")).unwrap();

        assert_eq!(result.unwrap().counts, expected);
    }

    include!(concat!(env!("OUT_DIR"), "/lib.include"));
}
