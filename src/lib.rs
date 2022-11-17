use std::collections::HashMap;
use std::error::Error;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::{panic, thread};

use crossbeam::channel::{bounded, Receiver};
use serde_json::Value;

pub struct Coverage {
    pub counts: HashMap<String, u32>,
}

impl Coverage {
    fn new() -> Self {
        Coverage {
            counts: HashMap::new()
        }
    }

    pub fn run(filename: PathBuf, threads: usize) -> Result<Coverage, Box<dyn Error>> {
        let file = File::open(filename)?;
        let reader = BufReader::new(file);
        // Use bounded() to prevent loading the entire file into memory. Memory usage looks okay with 1024.
        let (sender, receiver_) = bounded(1024);
        let mut handles = vec![];

        for _ in 0..threads {
            let receiver: Receiver<String> = receiver_.clone();

            handles.push(thread::spawn(|| {
                let mut coverage = Coverage::new();

                for line in receiver {
                    let value: Value = serde_json::from_str(&line)?;
                    coverage.add(value, &mut Vec::with_capacity(16));
                }

                Ok::<Coverage, serde_json::Error>(coverage)
            }));
        }

        for line in reader.lines() {
            sender.send(line?)?;
        }

        // Drop the sender, to close the channel.
        drop(sender);

        let mut total_coverage = Coverage::new();

        for handle in handles {
            match handle.join() {
                Ok(result) => {
                    if let Ok(coverage) = result {
                        for (k, v) in coverage.counts {
                            total_coverage.increment(k, v);
                        }
                    }
                },
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
