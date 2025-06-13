use std::fs::File;
use std::io::{self, BufReader, Read, Write};
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
    /// Correct quality issues within OCDS compiled releases in a line-delimited JSON file
    ///
    /// Corrected data is written to standard output as line-delimited JSON.
    ///
    /// Quality issues are written to standard error as CSV rows with the columns: line, ocid, path, array
    /// indexes, incorrect value, error description.
    Prepare {
        /// The path to the file (or "-" for standard input), in which each line is a contracting process as JSON text
        file: PathBuf,
        /// The path to the settings file
        #[arg(long, short, value_parser = settings_parser)]
        settings: Option<ocdscardinal::Settings>,
        /// The file to which to write corrected data (or "-" for standard output)
        #[arg(long, short)]
        output: PathBuf,
        /// The file to which to write quality issues (or "-" for standard output)
        #[arg(long, short)]
        errors: PathBuf,
    },
    /// Calculate procurement indicators from OCDS compiled releases in a line-delimited JSON file
    ///
    /// The result is a JSON object, in which the keys are one of `OCID`, `Buyer`, `ProcuringEntity`
    /// or `Tenderer`. The values are JSON objects, in which the keys are identifiers (e.g. ocid)
    /// and values are results (of any indicators that returned a result).
    ///
    /// Unless --no-meta is set, the result has a "Meta" key, with information about the quartiles
    /// and fences used to calculate the results.
    ///
    /// If --map is set, the result has a "Maps" key, with mappings from contracting processes to
    /// organizations.
    Indicators {
        /// The path to the file (or "-" for standard input), in which each line is a contracting process as JSON text
        file: PathBuf,
        /// The path to the settings file
        #[arg(long, short, value_parser = settings_parser)]
        settings: Option<ocdscardinal::Settings>,
        /// Print the number of results per group to standard error
        #[arg(long, short, default_value_t = false)]
        count: bool,
        /// Exclude the "Meta" key from the results object
        #[arg(long, default_value_t = false)]
        no_meta: bool,
        /// Include the "Maps" key, mapping contracting processes to organizations
        #[arg(long, default_value_t = false)]
        map: bool,
    },
    /// Write a default settings file for configuration.
    Init {
        /// The path to the settings file to write
        file: PathBuf,
        /// Overwrite the settings file if it already exists
        #[arg(long, short, default_value_t = false)]
        force: bool,
    },
}

fn file_argument_error(file: &Path, message: &str) -> ! {
    Cli::command()
        .error(ErrorKind::ValueValidation, format!("{}: {message}", file.display()))
        .exit()
}

fn settings_parser(s: &str) -> Result<ocdscardinal::Settings, ConfigError> {
    let config = Config::builder().add_source(config::File::with_name(s)).build()?;
    serde_path_to_error::deserialize(config).map_err(|error| match error.inner() {
        ConfigError::Message(_) => ConfigError::Message(error.to_string()),
        _ => error.into_inner(),
    })
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

fn create(file: &PathBuf) -> Box<dyn Write + Send> {
    if file == &PathBuf::from("-") {
        Box::new(io::stdout())
    } else {
        Box::new(File::create(file).unwrap_or_else(|e| file_argument_error(file, &e.to_string())))
    }
}

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
        Commands::Init { file, force } => match ocdscardinal::init(file, force) {
            Err(e) => eprintln!("Error writing to {}: {e}", file.display()),
            Ok(false) => println!("Settings written to {}.", file.display()),
            Ok(true) => {} // written to standard output
        },
        Commands::Coverage { file } => match ocdscardinal::Coverage::run(reader(file)) {
            Ok(item) => println!("{:?}", item.results()),
            Err(e) => application_error(&e),
        },
        Commands::Prepare {
            file,
            settings,
            output,
            errors,
        } => {
            if let Err(e) = ocdscardinal::Prepare::run(
                reader(file),
                settings.clone().unwrap_or_default(),
                &mut create(output),
                &mut create(errors),
            ) {
                application_error(&e);
            }
        }
        Commands::Indicators {
            file,
            count,
            settings,
            no_meta,
            map,
        } => match ocdscardinal::Indicators::run(reader(file), settings.clone().unwrap_or_default(), map) {
            Ok(item) => {
                let mut output = serde_json::to_value(item.results()).unwrap();
                if !no_meta {
                    output["Meta"] = serde_json::to_value(&item.meta).unwrap();
                }
                if *map {
                    output["Maps"] = serde_json::to_value(&item.maps).unwrap();
                }
                println!("{}", serde_json::to_string(&output).unwrap());
                if *count {
                    for (group, subresults) in item.results() {
                        eprintln!("{:?}: {:?}", group, subresults.len());
                    }
                }
            }
            Err(e) => application_error(&e),
        },
    }
}
