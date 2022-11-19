use std::path::PathBuf;
use std::process;

use clap::{Parser, Subcommand};
use human_panic::setup_panic;
use log::LevelFilter;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[arg(short, long, global = true, default_value_t = 1, action = clap::ArgAction::Count)]
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
    },
}

fn main() {
    setup_panic!();

    let cli = Cli::parse();

    let level = match cli.verbose {
        1 => LevelFilter::Warn,
        2 => LevelFilter::Info,
        3 => LevelFilter::Debug,
        _ => LevelFilter::Trace,
    };

    pretty_env_logger::formatted_builder()
        .filter_level(level)
        .init();

    match &cli.command {
        Commands::Coverage { file } => match libocdscardinal::Coverage::run(file.to_path_buf()) {
            Ok(coverage) => {
                println!("{:?}", coverage.counts);
            }
            Err(e) => {
                eprintln!("Application error: {e:#}");
                process::exit(1);
            }
        },
    }
}
