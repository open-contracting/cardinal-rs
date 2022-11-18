use std::process;
use std::path::PathBuf;

use clap::{Parser, Subcommand};
use log::LevelFilter;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// The number of threads to spawn (0 for one thread per CPU)
    #[arg(short, long, global = true, default_value_t = 0)]
    threads: usize,
    #[arg(short, long, global = true, action = clap::ArgAction::Count)]
    verbose: u8,
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

    let level = match cli.verbose {
        0 => LevelFilter::Error,
        1 => LevelFilter::Warn,
        2 => LevelFilter::Info,
        3 => LevelFilter::Debug,
        _ => LevelFilter::Trace,
    };

    pretty_env_logger::formatted_builder().filter_level(level).init();

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
