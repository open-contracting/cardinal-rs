use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::{panic, thread};

use anyhow::{Context, Result};
use crossbeam::channel::{bounded, Receiver};
use log::warn;
use serde_json::Value;

#[derive(Debug)]
pub struct Coverage {
    pub counts: HashMap<String, u32>,
}

impl Coverage {
    fn new() -> Self {
        Coverage {
            counts: HashMap::new()
        }
    }

    pub fn run(filename: PathBuf, threads: usize) -> Result<Coverage> {
        let file = File::open(&filename).context(format!("No such file '{}'", filename.display()))?;
        let reader = BufReader::new(file);
        // Use bounded() to prevent loading the entire file into memory. Memory usage looks okay with 1024.
        let (sender, receiver_) = bounded(1024);
        let mut handles = vec![];

        for _ in 0..threads {
            let receiver: Receiver<(usize, String)> = receiver_.clone();

            handles.push(thread::spawn(|| {
                let mut coverage = Coverage::new();

                for (i, string) in receiver {
                    match serde_json::from_str(&string) {
                        Ok(value) => coverage.add(value, &mut Vec::with_capacity(16)),
                        Err(e) => {
                            // Skip empty lines silently.
                            // https://stackoverflow.com/a/64361042/244258
                            if !string.as_bytes().iter().all(u8::is_ascii_whitespace) {
                                warn!("Line {} is invalid JSON, skipping. [{e}]", i + 1)
                            }
                        }
                    }
                }

                coverage
            }));
        }

        for (i, result) in reader.lines().enumerate() {
            match result {
                // Err: Channel disconnect or timeout.
                // https://docs.rs/crossbeam/latest/crossbeam/channel/struct.Sender.html#method.send
                Ok(string) => sender.send((i, string))?,
                // Err: https://doc.rust-lang.org/std/io/enum.ErrorKind.html
                // https://github.com/rust-lang/rust/blob/1.65.0/library/std/src/io/buffered/bufreader.rs#L362-L365
                Err(e) => warn!("Line {} caused an I/O error, skipping. [{e}]", i + 1),
            }
        }

        // Drop the sender, to close the channel.
        drop(sender);

        let mut total_coverage = Coverage::new();

        for handle in handles {
            match handle.join() {
                // TODO Can try to update entry across threads, using DashMap, Arc, etc.
                Ok(coverage) => {
                    for (k, v) in coverage.counts {
                        total_coverage.increment(k, v);
                    }
                },
                // Err: Associated thread panic.
                // https://doc.rust-lang.org/stable/std/thread/type.Result.html
                Err(e) => panic::resume_unwind(e),
            }
        }

        Ok(total_coverage)
    }

    // The longest path has 6 parts (as below or contracts/implementation/transactions/payer/identifier/id).
    // The longest pointer has 10 parts (contracts/0/amendments/0/unstructuredChanges/0/oldValue/classifications/0/id).
    fn add(&mut self, value: Value, path: &mut Vec<String>) {
        match value {
            Value::Null => {},
            Value::Array(vec) => {
                path.push(String::from("-"));
                for i in vec {
                    self.add(i, path);
                }
                path.pop();
            },
            // Note:
            // - "" keys and literal values yield the same path.
            // - If a member is repeated, the last is measured.
            Value::Object(map) => {
                for (k, v) in map {
                    path.push(k);
                    self.add(v, path);
                    path.pop();
                }
            },
            Value::String(string) => {
                if !string.is_empty() {
                    self.increment(path.join("/"), 1);
                }
            },
            _ => { // number, boolean
                // Using a String as the key with `join("/")` is faster than Vec<String> as the key with `to_vec()`.
                self.increment(path.join("/"), 1);
            },
        }
    }

    fn increment(&mut self, path: String, delta: u32) {
        self.counts.entry(path).and_modify(|count| { *count += delta }).or_insert(delta);
    }
}
