use std::fs::File;
use std::io::{self, BufReader, Read};
use std::path::{Path, PathBuf};
use std::process;

use clap::error::ErrorKind;
use clap::{CommandFactory, Parser, Subcommand};
use human_panic::setup_panic;
use log::LevelFilter;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// Increase verbosity.
    #[arg(short, long, global = true, default_value_t = 1, action = clap::ArgAction::Count)]
    verbose: u8,
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Count the number of times each field is non-empty in a line-delimited JSON file
    ///
    /// The command walks the JSON tree, counting non-empty nodes. Empty nodes are "", [], {} and null, and any nodes
    /// containing only empty nodes.
    ///
    /// The result is a JSON object, in which keys are paths and values are counts.
    ///
    /// The "" path corresponds to a line. A path ending with / corresponds to an object. A path ending with []
    /// corresponds to an array element. Other paths correspond to object members.
    // https://github.com/clap-rs/clap/issues/2389
    #[clap(verbatim_doc_comment)]
    Coverage {
        /// The path to the file containing OCDS data (or "-" for standard input), in which each line is a contracting process as JSON text
        file: PathBuf,
    },
}

fn error(file: &Path, message: &str) -> ! {
    Cli::command()
        .error(
            ErrorKind::ValueValidation,
            format!("{}: {message}", file.display()),
        )
        .exit()
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
        Commands::Coverage { file } => {
            let file: Box<dyn Read + Send> = if file == &PathBuf::from("-") {
                Box::new(io::stdin())
            } else {
                // If the file is replaced with a directory after this check, run() won't terminate.
                if file.is_dir() {
                    error(file, "Is a directory, not a file");
                }
                match File::open(file) {
                    Ok(file) => Box::new(file),
                    Err(e) => error(file, &e.to_string()),
                }
            };

            match libocdscardinal::Coverage::run(BufReader::new(file)) {
                Ok(coverage) => {
                    println!("{:?}", coverage.counts());
                }
                Err(e) => {
                    eprintln!("Application error: {e:#}");
                    process::exit(1);
                }
            }
        }
    }
}
