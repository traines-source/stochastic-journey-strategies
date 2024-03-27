use std::collections::HashMap;
use std::time::Duration;

use stost::connection;
use stost::connection::StopInfo;
use stost::distribution_store;
use stost::gtfs;
use stost::query::Query;
use stost::wire::serde;
use stost::query::topocsa;
use rustc_hash::FxHashSet;

use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn from_relevant(c: &mut Criterion) {
    let mut store = distribution_store::Store::new();
    store.load_distributions("./data/de_db.csv");

    let bytes: Vec<u8> = serde::read_protobuf("./tests/fixtures/basic.pb");
    let mut stations = vec![];
    let mut routes = vec![];
    let mut connections = vec![];
    let mut order = vec![];
    let mut cut = FxHashSet::default();
    let meta = serde::deserialize_protobuf(bytes, &mut stations, &mut routes, &mut connections, false);
    let mut env = topocsa::prepare(&mut store, &mut connections, &stations, &mut cut, &mut order, serde::to_mtime(meta.now, meta.start_ts), 0.0, true);

    let mut group = c.benchmark_group("once");
    //measurement_time(Duration::from_secs(10))
    group.bench_function("from_relevant", |b| b.iter(|| env.query(black_box(meta.origin_idx), black_box(meta.destination_idx), black_box(7200), black_box(8640))));
    group.finish();
}

fn measure_prepare(c: &mut Criterion) {
    let mut store = distribution_store::Store::new();
    store.load_distributions("./data/de_db.csv");

    let bytes: Vec<u8> = serde::read_protobuf("./tests/fixtures/basic.pb");
    let mut stations = vec![];
    let mut routes = vec![];
    let mut connections = vec![];
    let mut cut = FxHashSet::default();

    let meta = serde::deserialize_protobuf(bytes, &mut stations, &mut routes, &mut connections, false);

    let mut group = c.benchmark_group("once");
    group.bench_function("measure_prepare", |b| b.iter(|| {
        let mut order = vec![];
        topocsa::prepare(black_box(&mut store), black_box(&mut connections.clone()), black_box(&stations), black_box(&mut cut), black_box(&mut order), black_box(serde::to_mtime(meta.now, meta.start_ts)), black_box(0.0), black_box(true));
    }));
    group.finish();
}

fn from_gtfs(c: &mut Criterion) {
    let mut store = distribution_store::Store::new();
    store.load_distributions("./data/ch_sbb.csv");

    let mut tt = gtfs::load_gtfs_cache("./tests/fixtures/timetable.ign.cache");
    let mut env = topocsa::Environment::new(&mut store, &mut tt.connections, &tt.stations, &mut tt.cut, &mut tt.order, 7500, 0.01, 0.01, true, false);
    let contr = gtfs::get_station_contraction(&tt.stations);
    env.set_station_contraction(&contr);
    let o = 10000;
    let d = 20000;
    println!("querying...");
    let mut group = c.benchmark_group("once");
    group.sample_size(10);
    group.bench_function("from_gtfs", |b| b.iter(|| env.query(black_box(o), black_box(d), black_box(7500), black_box(8220))));
    group.finish();
}

fn before_probability(c: &mut Criterion) {
    let mut store = distribution_store::Store::new();
    store.load_distributions("./data/ch_sbb.csv");

    let arr = StopInfo::new(1000, Some(10));
    let a = store.delay_distribution(&arr, false, 1, 0);
    let dep = StopInfo::new(1020, None);
    let d = store.delay_distribution(&dep, true, 1, 0);

    let mut group = c.benchmark_group("once");
    group.bench_function("before_probability", |b| b.iter(|| a.before_probability(&d, 0)));
    group.finish();
}

criterion_group!(benches, from_relevant, from_gtfs, before_probability, measure_prepare);
//criterion_group!(benches, measure_prepare);

