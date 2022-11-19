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
    /// Count the number of times each field is non-empty in a line-delimited JSON file
    ///
    /// The command walks the JSON tree, counting non-empty nodes. Empty nodes are "", [], {} and null, and any nodes
    /// containing only empty nodes.
    ///
    /// The result is a JSON object, in which keys are paths and values are counts.
    ///
    /// The "" path corresponds to a line. A path ending with / corresponds to an object node. A path ending with []
    /// corresponds to an array element. Other paths correspond to object members.
    ///
    /// Caveats:
    /// - If a member name is duplicated, only the last duplicate is considered.
    ///
    ///       $ echo '{"a": 0, "a": null}' | libocdscardinal coverage
    ///       {}
    ///
    /// - If a member name is empty, its path is the same as its parent object's path:
    ///
    ///       $ echo '{"": 0}' | libocdscardinal coverage
    ///       {"": 1, "/": 2}
    ///
    /// - If a member name ends with [], its path can be the same as a matching sibling's path:
    ///
    ///       $ echo '{"a[]": 0, "a": [0]}' | libocdscardinal coverage
    ///       {"": 1, "/": 1, "/a": 1, "/a[]": 2}
    // https://github.com/clap-rs/clap/issues/2389
    #[clap(verbatim_doc_comment)]
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
                println!("{:?}", coverage.counts());
            }
            Err(e) => {
                eprintln!("Application error: {e:#}");
                process::exit(1);
            }
        },
    }
}
