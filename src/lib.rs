use std::collections::HashMap;
use std::error::Error;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::{panic, thread};

use crossbeam::channel::{bounded, Receiver};
use serde_json::Value;

pub struct Config {
    pub path: String,
    pub threads: usize,
}

impl Config {
    pub fn build(mut args: impl Iterator<Item = String>) -> Result<Config, &'static str> {
        args.next();

        let path = match args.next() {
            Some(arg) => arg,
            None => return Err("Missing argument 'FILENAME'."),
        };

        // TODO Make configurable.
        let threads = num_cpus::get();

        Ok(Config {
            path,
            threads,
        })
    }
}

pub struct Coverage {
    pub counts: HashMap<String, u32>,
}

impl Coverage {
    fn new() -> Self {
        Coverage {
            counts: HashMap::new()
        }
    }

    pub fn build(config: Config) -> Result<Coverage, Box<dyn Error>> {
        // TODO Understand why this annotation is required.
        let mut threads: Vec<thread::JoinHandle<Result<Coverage, serde_json::Error>>> = vec![];
        let (sender, receiver_) = bounded(1024);

        for _ in 0..config.threads {
            let receiver: Receiver<String> = receiver_.clone();

            threads.push(thread::spawn(|| {
                let mut coverage = Coverage::new();

                for line in receiver {
                    let value: Value = serde_json::from_str(&line)?;
                    if value.is_object() {
                        coverage.add(value, &mut Vec::with_capacity(16));
                    } else {
                        // TODO return Err() feedback about format
                    }
                }

                Ok(coverage)
            }));
        }

        let file = File::open(config.path)?;
        let reader = BufReader::new(file);

        for line in reader.lines() {
            sender.send(line?)?;
        }

        // Drop the sender, to close the channel.
        drop(sender);

        let mut total_coverage = Coverage::new();

        for thread in threads {
            match thread.join() {
                Ok(result) => {
                    if let Ok(coverage) = result {
                        for (k, v) in coverage.counts {
                            total_coverage.counts.entry(k).and_modify(|count| { *count += v }).or_insert(v);
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
            Value::Array(vec) => {
                for i in vec {
                    self.add(i, path);
                }
            },
            Value::Object(map) => {
                for (k, v) in map {
                    path.push(k);
                    self.add(v, path);
                    path.pop();
                }
            },
            _ => { // string, number, boolean, null
                // String as key with `join("/")` is faster than Vec<String> as key with `to_vec()`.
                self.counts.entry(path.join("/")).and_modify(|count| { *count += 1 }).or_insert(1);
            },
        }
    }
}

pub fn run(config: Config) -> Result<(), Box<dyn Error>> {
    let coverage = Coverage::build(config)?;

    println!("{:?}", coverage.counts);

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn t() {
    }
}
