#![feature(unix_sigpipe)]

use std::fs::File;
use std::io::{self, BufReader, Read};
use std::path::{Path, PathBuf};
use std::process;

use clap::error::ErrorKind;
use clap::{CommandFactory, Parser, Subcommand};
use config::{Config, ConfigError};
use human_panic::setup_panic;
use log::LevelFilter;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// Increase verbosity
    #[arg(long, short, global = true, default_value_t = 1, action = clap::ArgAction::Count)]
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
    Coverage {
        /// The path to the file (or "-" for standard input), in which each line is JSON text
        file: PathBuf,
    },
    ///
    Prepare {
        /// The path to the file (or "-" for standard input), in which each line is a contracting process as JSON text
        file: PathBuf,
        /// The path to the settings file
        #[arg(long, short, value_parser = settings_parser)]
        settings: Option<ocdscardinal::Settings>,
    },
    /// Calculate procurement indicators from OCDS compiled releases in a line-delimited JSON file
    ///
    /// The result is a JSON object, in which the keys are one of "OCID", "Buyer", "ProcuringEntity"
    /// or "Tenderer". The values are JSON objects, in which the keys are identifiers (e.g. ocid)
    /// and values are results (of any indicators that returned a result).
    Indicators {
        /// The path to the file (or "-" for standard input), in which each line is a contracting process as JSON text
        file: PathBuf,
        /// Print the number of OCIDs with results
        #[arg(long, short, default_value_t = false)]
        count: bool,
        /// The path to the settings file
        #[arg(long, short, value_parser = settings_parser)]
        settings: Option<ocdscardinal::Settings>,
    },
    /// Write a default settings file for configuration.
    Init {
        /// The path to the settings file to write
        file: PathBuf,
    },
}

fn file_argument_error(file: &Path, message: &str) -> ! {
    Cli::command()
        .error(ErrorKind::ValueValidation, format!("{}: {message}", file.display()))
        .exit()
}

fn settings_parser(s: &str) -> Result<ocdscardinal::Settings, ConfigError> {
    Config::builder()
        .add_source(config::File::with_name(s))
        .build()?
        .try_deserialize::<ocdscardinal::Settings>()
}

fn application_error(e: &anyhow::Error) -> ! {
    eprintln!("Application error: {e:#}");
    process::exit(1);
}

fn reader(file: &PathBuf) -> BufReader<Box<dyn Read + Send>> {
    if file == &PathBuf::from("-") {
        BufReader::new(Box::new(io::stdin()))
    } else {
        // If the file is replaced with a directory after this check, run() won't terminate.
        if file.is_dir() {
            file_argument_error(file, "Is a directory, not a file");
        }
        match File::open(file) {
            Ok(file) => BufReader::new(Box::new(file)),
            Err(e) => file_argument_error(file, &e.to_string()),
        }
    }
}

#[unix_sigpipe = "sig_dfl"]
fn main() {
    setup_panic!();

    let cli = Cli::parse();

    // Can use https://github.com/clap-rs/clap-verbosity-flag if needed.
    let level = match cli.verbose {
        1 => LevelFilter::Warn,
        2 => LevelFilter::Info,
        3 => LevelFilter::Debug,
        _ => LevelFilter::Trace,
    };

    pretty_env_logger::formatted_builder().filter_level(level).init();

    match &cli.command {
        Commands::Init { file } => match ocdscardinal::init(file) {
            Err(e) => {
                eprintln!("Error writing to {file:?}: {e}");
            }
            Ok(false) => {
                println!("Settings written to {file:?}.");
            }
            _ => {}
        },
        Commands::Coverage { file } => match ocdscardinal::Coverage::run(reader(file)) {
            Ok(item) => {
                println!("{:?}", item.results());
            }
            Err(e) => {
                application_error(&e);
            }
        },
        Commands::Prepare { file, settings } => {
            ocdscardinal::Prepare::run(reader(file), settings.clone().unwrap_or_default());
        }
        Commands::Indicators { file, count, settings } => {
            match ocdscardinal::Indicators::run(reader(file), settings.clone().unwrap_or_default()) {
                Ok(item) => {
                    println!("{}", serde_json::to_string(&item.results()).unwrap());
                    if *count {
                        for (group, subresults) in item.results() {
                            println!("{:?}: {:?}", group, subresults.len());
                        }
                    }
                }
                Err(e) => {
                    application_error(&e);
                }
            }
        }
    }
}
