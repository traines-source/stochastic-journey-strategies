#[macro_use]
extern crate assert_float_eq;
extern crate rmp_serde as rmps;

use std::collections::HashMap;
use std::collections::HashSet;
use serde::de;
use serde::{Serialize, Deserialize};
use rmps::Serializer;
use std::io::Write;
use std::fs;

use stost::distribution_store;
use stost::query::topocsa;
use stost::gtfs;
use stost::connection;

#[derive(Serialize, Deserialize, Debug)]
struct Timetable {
    stations: Vec<connection::Station>,
    connections: Vec<connection::Connection>,
    cut: HashSet<(usize, usize)>,
    labels: HashMap<usize, topocsa::ConnectionLabel>,
    transport_and_day_to_connection_id: HashMap<(usize, u16), usize>
}

const CACHE_PATH: &str = "./tests/fixtures/timetable.ign.cache";
const GTFS_PATH: &str = "/gtfs/swiss-gtfs/2023-11-06/";
const GTFSRT_PATH: &str = "/gtfs/swiss-gtfs-rt/2023-11-06/";

fn day(year: i32, month: u32, day: u32) -> chrono::NaiveDate {
    chrono::NaiveDate::from_ymd_opt(year, month, day).unwrap()
}

#[test]
#[ignore]
fn create_gtfs_cache() {
    let mut store = distribution_store::Store::new();
    store.load_distributions("./data/ch_sbb.csv");

    let mut tt = Timetable {
        stations: vec![],
        connections: vec![],
        cut: HashSet::new(),
        labels: HashMap::new(),
        transport_and_day_to_connection_id: HashMap::new()
    };
    let mut routes = vec![];
    let t = gtfs::load_timetable(GTFS_PATH, day(2023, 11, 1), day(2023, 11, 2));
    tt.transport_and_day_to_connection_id = gtfs::retrieve(&t, &mut tt.stations, &mut routes, &mut tt.connections);
    let env = topocsa::prepare(&mut store, &mut tt.connections, &tt.stations, 0, 0.01, true);
    tt.cut = env.cut;
    tt.labels = env.labels;
    let mut buf = vec![];
    tt.serialize(&mut Serializer::new(&mut buf)).unwrap();
    let mut file = fs::OpenOptions::new()
        .create(true)
        .write(true)
        .open(CACHE_PATH).expect("file not openable");
    file.write_all(&buf).expect("error writing file");
}

fn load_gtfs_cache() -> Timetable {
    let buf = std::fs::read(CACHE_PATH).unwrap();
    rmp_serde::from_slice(&buf).unwrap()
}

#[test]
#[ignore]
fn gtfs() {
    let mut store = distribution_store::Store::new();
    store.load_distributions("./data/ch_sbb.csv");

    let mut tt = load_gtfs_cache();
    let mut env = topocsa::new(&mut store, &mut tt.connections, &tt.stations, tt.cut, tt.labels, 0, 0.0, true);
    let o = 10000;
    let d = 20000;
    println!("querying...");
    let station_labels = env.query(&tt.stations[o], &tt.stations[d]);
    let origin_deps = &station_labels[&o];
    let best_conn = &tt.connections[*origin_deps.last().unwrap()];
    let second_best_conn = &tt.connections[origin_deps[origin_deps.len()-2]];
    println!("{:?} {:?} {:?} {:?} {:?}{:?}", tt.stations[o].name, tt.stations[d].name, best_conn.departure, best_conn.destination_arrival, second_best_conn.departure, second_best_conn.destination_arrival);
}

#[test]
#[ignore]
fn gtfs_with_rt() {
    let mut store = distribution_store::Store::new();
    store.load_distributions("./data/ch_sbb.csv");

    let mut tt = load_gtfs_cache();
    let t = gtfs::load_timetable(GTFS_PATH, day(2023, 11, 1), day(2023, 11, 2));
    let mut env = topocsa::new(&mut store, &mut tt.connections, &tt.stations, tt.cut, tt.labels, 0, 0.0, true);
    let path = format!("{}2023-11-01T00:00:02+01:00.gtfsrt", GTFSRT_PATH);
    gtfs::load_realtime(&path, &t, tt.transport_and_day_to_connection_id,
        |connection_id: usize, is_departure: bool, delay: i16, cancelled: bool| env.update(connection_id, is_departure, delay, cancelled)
    );

    let o = 10000;
    let d = 20000;
    println!("querying...");
    let station_labels = env.query(&tt.stations[o], &tt.stations[d]);
    let origin_deps = &station_labels[&o];
    let best_conn = &tt.connections[*origin_deps.last().unwrap()];
    let second_best_conn = &tt.connections[origin_deps[origin_deps.len()-2]];
    println!("{:?} {:?} {:?} {:?} {:?}{:?}", tt.stations[o].name, tt.stations[d].name, best_conn.departure, best_conn.destination_arrival, second_best_conn.departure, second_best_conn.destination_arrival);
}

#[test]
fn gtfs_small() {
    let mut store = distribution_store::Store::new();
    store.load_distributions("./data/ch_sbb.csv");

    let mut stations = vec![];
    let mut routes = vec![];
    let mut connections = vec![];
    let t = gtfs::load_timetable("./tests/fixtures/gtfs_minimal_swiss/", day(2024, 1, 1), day(2024, 1, 10));
    let map = gtfs::retrieve(&t, &mut stations, &mut routes, &mut connections);

    let mut env = topocsa::prepare(&mut store, &mut connections, &stations, 0, 0.01, false);
    gtfs::load_realtime("./tests/fixtures/2024-01-02T01_48_02+01_00.gtfsrt", &t, map, 
        |connection_id: usize, is_departure: bool, delay: i16, cancelled: bool| env.update(connection_id, is_departure, delay, cancelled)
    );

    let o = 11;
    let d = 69;
    println!("{:?} {:?} {:?} {:?}", stations[o].id, stations[o].name, stations[d].id, stations[d].name);
    assert_eq!(stations[d].footpaths.len(), 3);

    let station_labels = env.query(&stations[o], &stations[d]);
    let origin_deps = &station_labels[&o];
    let best_conn = &connections[*origin_deps.last().unwrap()];
    let second_best_conn = &connections[origin_deps[origin_deps.len()-2]];
    println!("{:?} {:?} {:?} {:?} {:?}{:?}", stations[o].name, stations[d].name, best_conn.departure, best_conn.destination_arrival, second_best_conn.departure, second_best_conn.destination_arrival);
    
}

