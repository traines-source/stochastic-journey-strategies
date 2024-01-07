use std::collections::HashMap;
use std::time::Duration;

use stost::connection;
use stost::distribution_store;
use stost::wire::serde;
use stost::query::topocsa;

use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn criterion_benchmark(c: &mut Criterion) {
    let mut store = distribution_store::Store::new();
    store.load_distributions("./data/de_db.csv");

    let bytes: Vec<u8> = serde::read_protobuf("./tests/fixtures/basic.pb");
    let mut stations = vec![];
    let mut routes = vec![];
    let mut connections = vec![];
    let (start_time, o, d, now, _) = serde::deserialize_protobuf(bytes, &mut stations, &mut routes, &mut connections, false);
    let mut env = topocsa::prepare(&mut store, &mut connections, &stations, serde::to_mtime(now, start_time), 0.0, true);

    let mut group = c.benchmark_group("once");
    group.sample_size(10); //measurement_time(Duration::from_secs(10))
    group.bench_function("basic", |b| b.iter(|| env.query(black_box(&stations[o]), black_box(&stations[d]))));
    group.finish();
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);