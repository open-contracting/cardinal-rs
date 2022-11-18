use std::error::Error;
use std::fs::File;
use std::io::{BufReader, ErrorKind};
use std::path::{Path, PathBuf};

use serde_json;
use pretty_assertions::assert_eq;

#[test]
fn test_nonexistent() {
    let result = libocdscardinal::Coverage::run(PathBuf::from("nonexistent"), 1);
    let error = result.unwrap_err();

    // https://docs.rs/anyhow/latest/anyhow/struct.Error.html#display-representations
    assert_eq!(format!("{:#}", error), "No such file 'nonexistent': No such file or directory (os error 2)");
    // https://github.com/dtolnay/anyhow/blob/1.0.66/tests/test_downcast.rs#L66-L69
    assert_eq!(error.downcast::<std::io::Error>().unwrap().kind(), ErrorKind::NotFound);
}

fn check(input: &str, output: &str) -> Result<(), Box<dyn Error>> {
    let fixtures = Path::new("tests/fixtures");

    let inpath = fixtures.join(input);
    let result = libocdscardinal::Coverage::run(inpath, 2);

    let outpath = fixtures.join(output);
    let file = File::open(outpath)?;
    let reader = BufReader::new(file);
    let expected = serde_json::from_reader(reader)?;

    assert_eq!(result.unwrap().counts, expected);

    Ok(())
}

include!(concat!(env!("OUT_DIR"), "/tests.rs"));
