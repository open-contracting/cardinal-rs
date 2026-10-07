#![feature(custom_test_frameworks)]
#![test_runner(criterion::runner)]

use std::fs::File;
use std::hint::black_box;
use std::io::BufReader;
use std::time::Duration;

use criterion::Criterion;
use criterion_macro::criterion;

use ocdscardinal::indicators::{Empty, FloatThreshold, IntegerThreshold, R003, R025, R038, R048};
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
                    R003: Some(R003::default()),
                    R024: Some(FloatThreshold::default()),
                    R025: Some(R025::default()),
                    R028: Some(Empty::default()),
                    R030: Some(Empty::default()),
                    R035: Some(IntegerThreshold::default()),
                    R036: Some(Empty::default()),
                    R038: Some(R038::default()),
                    R048: Some(R048::default()),
                    R058: Some(FloatThreshold::default()),
                    ..Default::default()
                },
                &false,
            );
        });
    });

    group.finish();
}
