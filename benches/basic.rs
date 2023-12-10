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
    let mut stations: HashMap<String, connection::Station> = HashMap::new();
    let mut routes = HashMap::new();
    let mut connections = vec![];
    let (start_time, o, d, now) = serde::deserialize_protobuf(bytes, &mut stations, &mut routes, &mut connections);
    let mut env = topocsa::prepare(&mut store, &mut connections, serde::to_mtime(start_time, start_time), 0.0);

    let mut group = c.benchmark_group("once");
    group.measurement_time(Duration::from_secs(10)).sample_size(10);
    group.bench_function("basic", |b| b.iter(|| env.query(black_box(d))));
    group.finish();
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);