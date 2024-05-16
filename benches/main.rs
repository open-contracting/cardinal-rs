#![feature(custom_test_frameworks)]
#![test_runner(criterion::runner)]

use std::fs::File;
use std::io::BufReader;
use std::time::Duration;

use criterion::{black_box, Criterion};
use criterion_macro::criterion;

use ocdscardinal::{Indicators, Settings};

#[criterion]
fn bench(c: &mut Criterion) {
    let mut group = c.benchmark_group("group");

    group.measurement_time(Duration::from_secs(10)).sample_size(60); // defaults 5, 100

    group.bench_function("indicators", |b| {
        b.iter(|| {
            let path = "benches/fixtures/10000.jsonl";
            let file = File::open(path).unwrap();

            let _ = Indicators::run(
                black_box(BufReader::new(file)),
                Settings {
                    R003: Some(Default::default()),
                    R024: Some(Default::default()),
                    R025: Some(Default::default()),
                    R028: Some(Default::default()),
                    R030: Some(Default::default()),
                    R035: Some(Default::default()),
                    R036: Some(Default::default()),
                    R038: Some(Default::default()),
                    R048: Some(Default::default()),
                    R058: Some(Default::default()),
                    ..Default::default()
                },
                &false,
            );
        })
    });

    group.finish();
}
