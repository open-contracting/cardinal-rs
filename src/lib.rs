use std::collections::HashMap;
use std::error::Error;
use std::fs::File;
use std::io::{BufRead, BufReader};

use serde_json::Value;

pub struct Config {
    pub path: String,
}

impl Config {
    pub fn build(args: &[String]) -> Result<Config, &'static str> {
        if args.len() < 2 {
            return Err("not enough arguments");
        }

        let path = args[1].clone();

        Ok(Config {
            path
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
        // Compiled releases of multiple MiBs have been observed, but most are less than 1 MiB.
        const CAPACITY: usize = 1024 * 1024;

        let file = File::open(config.path)?;
        let mut line = Vec::with_capacity(CAPACITY);
        let mut reader = BufReader::new(file);
        let mut coverage = Coverage::new();

        while reader.read_until(b'\n', &mut line).unwrap_or(0) > 0 {
            let value: Value = serde_json::from_slice(&line)?;

            if value.is_object() {
                coverage.add(value, &mut Vec::with_capacity(16));
            } else {
                // TODO return Err() feedback about format
            }

            line.clear();
        }

        Ok(coverage)
    }

    // The longest path has 6 parts (as above or contracts/implementation/transactions/payer/identifier/id).
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
    let coverage = Coverage::build(config).unwrap();

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
