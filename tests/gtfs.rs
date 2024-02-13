#[macro_use]
extern crate rmp_serde as rmps;

use std::collections::HashMap;
use rustc_hash::FxHashSet;
use std::env;
use serde::Serialize;
use rmps::Serializer;
use std::io::Write;
use std::fs;
use stost::distribution_store;
use stost::query::topocsa;
use stost::gtfs;
use std::time::Instant;
const CACHE_PATH: &str = "./tests/fixtures/timetable.ign.cache";
const GTFS_PATH: &str = "/gtfs/swiss-gtfs/2023-11-06/";
const GTFSRT_PATH: &str = "/gtfs/swiss-gtfs-rt/2023-11-01/";

fn day(year: i32, month: u32, day: u32) -> chrono::NaiveDate {
    chrono::NaiveDate::from_ymd_opt(year, month, day).unwrap()
}

#[test]
#[ignore]
fn create_gtfs_cache() {
    let prefix = match env::var("STOST_GTFS_PATH") {
        Ok(v) => v,
        Err(_) => "".to_owned()
    };
    let mut store = distribution_store::Store::new();
    store.load_distributions("./data/ch_sbb.csv");

    let mut tt = gtfs::GtfsTimetable {
        stations: vec![],
        connections: vec![],
        cut: FxHashSet::default(),
        order: vec![],
        transport_and_day_to_connection_id: HashMap::new()
    };
    let mut routes = vec![];
    let t = gtfs::load_timetable(&format!("{}{}", prefix, GTFS_PATH), day(2023, 11, 2), day(2023, 11, 3));
    tt.transport_and_day_to_connection_id = gtfs::retrieve(&t, &mut tt.stations, &mut routes, &mut tt.connections);
    let start_ts = Instant::now();
    let env = topocsa::prepare(&mut store, &mut tt.connections, &tt.stations, &mut tt.order, 8000, 0.01, true);
    println!("elapsed: {}", start_ts.elapsed().as_millis());
    tt.cut = env.cut;
    let mut buf = vec![];
    tt.serialize(&mut Serializer::new(&mut buf)).unwrap();
    let mut file = fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(CACHE_PATH).expect("file not openable");
    file.write_all(&buf).expect("error writing file");
    /*
    let mut tt = gtfs::GtfsTimetable {
        stations: vec![],
        connections: vec![],
        cut: HashSet::new(),
        order: HashMap::new(),
        transport_and_day_to_connection_id: HashMap::new()
    };
    let mut routes = vec![];
    let t = gtfs::load_timetable(&format!("{}{}", prefix, GTFS_PATH), day(2023, 11, 2), day(2023, 11, 3));
    tt.transport_and_day_to_connection_id = gtfs::retrieve(&t, &mut tt.stations, &mut routes, &mut tt.connections);
    let start_ts = Instant::now();
    let env = topocsa::prepare(&mut store, &mut tt.connections, &tt.stations, &mut tt.order, 0, 0.01, true);
    println!("elapsed hot run: {}", start_ts.elapsed().as_millis());
    println!("cut {}", env.cut.len());
    */
}

#[test]
#[ignore]
fn create_simulation_samples() {
    let samples = gtfs::create_simulation_samples(GTFS_PATH, day(2023, 11, 2), day(2023, 11, 9));
    let buf = serde_json::to_vec(&samples).unwrap();
    let mut file = fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open("./benches/samples/samples.ign.json").expect("file not openable");
    file.write_all(&buf).expect("error writing file");
}

#[test]
#[ignore]
fn gtfs() {
    let mut store = distribution_store::Store::new();
    store.load_distributions("./data/ch_sbb.csv");

    let mut tt = gtfs::load_gtfs_cache(CACHE_PATH);
    let mut env = topocsa::new(&mut store, &mut tt.connections, &tt.stations, tt.cut, &mut tt.order, 0, 0.01, true);
    let o = 10000;
    let d = 20000;
    println!("querying...");
    let station_labels = env.query(&tt.stations[o], &tt.stations[d]);
    let origin_deps = &station_labels[o];
    let best_conn = origin_deps.last().unwrap();
    let second_best_conn = &origin_deps[origin_deps.len()/3];
    println!("{:?} {:?} {:?} {:?} {:?}{:?}", tt.stations[o].name, tt.stations[d].name, &tt.connections[best_conn.connection_idx].departure, best_conn.destination_arrival, &tt.connections[second_best_conn.connection_idx].departure, second_best_conn.destination_arrival);
}

#[test]
#[ignore]
fn gtfs_with_rt() {
    let mut store = distribution_store::Store::new();
    store.load_distributions("./data/ch_sbb.csv");

    let mut tt = gtfs::load_gtfs_cache(CACHE_PATH);
    let t = gtfs::load_timetable(GTFS_PATH, day(2023, 11, 1), day(2023, 11, 2));
    let mut env = topocsa::new(&mut store, &mut tt.connections, &tt.stations, tt.cut, &mut tt.order, 0, 0.01, true);
    let path = format!("{}2023-11-01T16:00:02+01:00.gtfsrt", GTFSRT_PATH);
    gtfs::load_realtime(&path, &t, &tt.transport_and_day_to_connection_id,
        |connection_id: usize, is_departure: bool, delay: i16, cancelled: bool| env.update(connection_id, is_departure, delay, cancelled)
    );

    let o = 10000;
    let d = 20000;
    println!("querying...");
    let station_labels = env.query(&tt.stations[o], &tt.stations[d]);
    let origin_deps = &station_labels[o];
    let best_conn = origin_deps.last().unwrap();
    let second_best_conn = &origin_deps[origin_deps.len()/3];
    println!("{:?} {:?} {:?} {:?} {:?}{:?}", tt.stations[o].name, tt.stations[d].name, &tt.connections[best_conn.connection_idx].departure, best_conn.destination_arrival, &tt.connections[second_best_conn.connection_idx].departure, second_best_conn.destination_arrival);
}

#[test]
#[ignore]
fn load_only_gtfs_with_rt() {
    let mut store = distribution_store::Store::new();
    store.load_distributions("./data/ch_sbb.csv");

    let mut tt = gtfs::GtfsTimetable {
        stations: vec![],
        connections: vec![],
        cut: FxHashSet::default(),
        order: vec![],
        transport_and_day_to_connection_id: HashMap::new()
    };
    let mut routes = vec![];
    let t = gtfs::load_timetable("/gtfs/swiss-gtfs/2024-01-15", day(2023, 1, 15), day(2024, 1, 16));
    let mapping = gtfs::retrieve(&t, &mut tt.stations, &mut routes, &mut tt.connections);
    let path = "/gtfs/swiss-gtfs-rt/2024-01-15/2024-01-15T01:32:01+01:00.gtfsrt";
    let mut env = topocsa::new(&mut store, &mut tt.connections, &tt.stations, tt.cut, &mut tt.order, 0, 0.01, true);
    gtfs::load_realtime(&path, &t, &mapping,
        |connection_id: usize, is_departure: bool, delay: i16, cancelled: bool| env.update(connection_id, is_departure, delay, cancelled)
    );

    let o = 10000;
    let d = 20000;
    println!("querying...");
    let station_labels = env.query(&tt.stations[o], &tt.stations[d]);
    let origin_deps = &station_labels[o];
    let best_conn = origin_deps.last().unwrap();
    let second_best_conn = &origin_deps[origin_deps.len()/3];
    println!("{:?} {:?} {:?} {:?} {:?}{:?}", tt.stations[o].name, tt.stations[d].name, &tt.connections[best_conn.connection_idx].departure, best_conn.destination_arrival, &tt.connections[second_best_conn.connection_idx].departure, second_best_conn.destination_arrival);
}

#[test]
fn gtfs_small() {
    let mut store = distribution_store::Store::new();
    store.load_distributions("./data/ch_sbb.csv");

    let mut stations = vec![];
    let mut routes = vec![];
    let mut connections = vec![];
    let mut order = vec![];
    let t = gtfs::load_timetable("./tests/fixtures/gtfs_minimal_swiss/", day(2024, 1, 1), day(2024, 1, 10));
    let map = gtfs::retrieve(&t, &mut stations, &mut routes, &mut connections);

    let mut env = topocsa::prepare(&mut store, &mut connections, &stations, &mut order, 0, 0.01, false);
    gtfs::load_realtime("./tests/fixtures/2024-01-02T01_48_02+01_00.gtfsrt", &t, &map, 
        |connection_id: usize, is_departure: bool, delay: i16, cancelled: bool| env.update(connection_id, is_departure, delay, cancelled)
    );

    let o = 11;
    let d = 69;
    println!("{:?} {:?} {:?} {:?}", stations[o].id, stations[o].name, stations[d].id, stations[d].name);
    assert_eq!(stations[d].footpaths.len(), 3);

    let station_labels = env.query(&stations[o], &stations[d]);
    let origin_deps = &station_labels[o];
    let best_conn = origin_deps.last().unwrap();
    let second_best_conn = &origin_deps[origin_deps.len()/3];
    println!("{:?} {:?} {:?} {:?} {:?}{:?}", stations[o].name, stations[d].name, &connections[best_conn.connection_idx].departure, best_conn.destination_arrival, &connections[second_best_conn.connection_idx].departure, second_best_conn.destination_arrival);
    
}

