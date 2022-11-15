use std::env;
use std::process;

use ocdscardinallib::Config;

fn main() {
    let config = Config::build(env::args()).unwrap_or_else(|e| {
        eprintln!("Error: {e}");
        process::exit(1);
    });

    if let Err(e) = ocdscardinallib::run(config) {
        eprintln!("Application error: {e}");
        process::exit(1);
    }

    // TODO handle errors returned by the ? in lib.rs (file I/O, JSON parsing)
    // https://doc.rust-lang.org/book/ch09-02-recoverable-errors-with-result.html#matching-on-different-errors
}
