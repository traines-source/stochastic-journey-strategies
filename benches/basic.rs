use std::collections::HashMap;
use std::time::Duration;

use stost::connection;
use stost::distribution_store;
use stost::gtfs;
use stost::wire::serde;
use stost::query::topocsa;

use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn from_relevant(c: &mut Criterion) {
    let mut store = distribution_store::Store::new();
    store.load_distributions("./data/de_db.csv");

    let bytes: Vec<u8> = serde::read_protobuf("./tests/fixtures/basic.pb");
    let mut stations = vec![];
    let mut routes = vec![];
    let mut connections = vec![];
    let mut order = HashMap::with_capacity(connections.len());
    let (start_time, o, d, now, _) = serde::deserialize_protobuf(bytes, &mut stations, &mut routes, &mut connections, false);
    let mut env = topocsa::prepare(&mut store, &mut connections, &stations, &mut order, serde::to_mtime(now, start_time), 0.0, true);

    let mut group = c.benchmark_group("once");
    group.sample_size(10); //measurement_time(Duration::from_secs(10))
    group.bench_function("basic", |b| b.iter(|| env.query(black_box(&stations[o]), black_box(&stations[d]))));
    group.finish();
}

fn from_gtfs(c: &mut Criterion) {
    let mut store = distribution_store::Store::new();
    store.load_distributions("./data/ch_sbb.csv");

    let mut tt = gtfs::load_gtfs_cache("./tests/fixtures/timetable.ign.cache");
    let mut env = topocsa::new(&mut store, &mut tt.connections, &tt.stations, tt.cut, &mut tt.labels, 0, 0.01, true);
    let o = 10000;
    let d = 20000;
    println!("querying...");
    let mut group = c.benchmark_group("once");
    group.sample_size(10);
    group.bench_function("basic", |b| b.iter(|| env.query(black_box(&tt.stations[o]), black_box(&tt.stations[d]))));
    group.finish();
}

criterion_group!(benches, from_relevant, from_gtfs);

