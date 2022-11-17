use std::error::Error;
use std::fs::File;
use std::io::BufReader;
use std::path::Path;

use serde_json;
use pretty_assertions::assert_eq;

fn check(input: &str, output: &str) -> Result<(), Box<dyn Error>> {
    let fixtures = Path::new("tests/fixtures");

    if output.starts_with("invalid_") {
        // TODO
    } else {
        let outpath = fixtures.join(output);
        let file = File::open(outpath)?;
        let reader = BufReader::new(file);
        let expected = serde_json::from_reader(reader)?;

        let inpath = fixtures.join(input);
        let coverage = libocdscardinal::Coverage::run(inpath, 2)?;

        assert_eq!(coverage.counts, expected);
    }

    Ok(())
}

include!(concat!(env!("OUT_DIR"), "/tests.rs"));
