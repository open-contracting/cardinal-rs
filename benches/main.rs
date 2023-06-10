#![feature(custom_test_frameworks)]
#![test_runner(criterion::runner)]

use std::fs::File;
use std::io::BufReader;

use criterion::{black_box, Criterion};
use criterion_macro::criterion;

use ocdscardinal::{Indicators, Settings};

#[criterion]
fn criterion_benchmark(c: &mut Criterion) {
    c.bench_function("indicators", |b| {
        b.iter(|| {
            let path = "tests/fixtures/indicators/R024.jsonl";
            let file = File::open(path).unwrap();

            let _ = Indicators::run(
                black_box(BufReader::new(file)),
                Settings {
                    R024: Some(Default::default()),
                    R025: Some(Default::default()),
                    R035: Some(Default::default()),
                    R036: Some(Default::default()),
                    R038: Some(Default::default()),
                    ..Default::default()
                },
            );
        })
    });
}
