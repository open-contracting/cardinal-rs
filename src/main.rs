use std::process;
use std::path::PathBuf;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// The number of threads to spawn (0 for one thread per CPU)
    #[arg(short, long, default_value_t = 0)]
    threads: usize,
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Count the number of times each field is set
    Coverage {
        /// The path to the file containing OCDS data, in which each line is a contracting process as JSON text
        file: PathBuf,
    }
}

fn main() {
    let cli = Cli::parse();

    match &cli.command {
        Commands::Coverage { file } => {
            let threads = if cli.threads == 0 {
                num_cpus::get()
            } else {
                cli.threads
            };

            match libocdscardinal::Coverage::run(file.to_path_buf(), threads) {
                Ok(coverage) => {
                    println!("{:?}", coverage.counts);
                }
                Err(e) => {
                    eprintln!("Application error: {e}");
                    process::exit(1);
                }
            }
        },
    }

    // TODO handle errors returned by the ? in lib.rs (file I/O, JSON parsing)
    // https://doc.rust-lang.org/book/ch09-02-recoverable-errors-with-result.html#matching-on-different-errors
}
