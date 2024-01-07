#[macro_use]
extern crate assert_float_eq;
extern crate rmp_serde as rmps;

use std::collections::HashSet;
use serde::{Serialize, Deserialize};
use rmps::{Deserializer, Serializer};
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
    cut: HashSet<(usize, usize)>
}

static CACHE_PATH: &str = "./tests/fixtures/timetable.ign.cache";

#[test]
#[ignore]
fn create_gtfs_cache() {
    let mut store = distribution_store::Store::new();
    store.load_distributions("./data/ch_sbb.csv");

    let mut tt = Timetable {
        stations: vec![],
        connections: vec![],
        cut: HashSet::new()
    };
    let mut routes = vec![];
    gtfs::load("/gtfs/swiss-gtfs/2023-11-06/", chrono::NaiveDate::from_ymd_opt(2023, 11, 1).unwrap(), chrono::NaiveDate::from_ymd_opt(2023, 11, 2).unwrap(), &mut tt.stations, &mut routes, &mut tt.connections);
    let env = topocsa::prepare(&mut store, &mut tt.connections, &tt.stations, 0, 0.01, true);
    tt.cut = env.cut;
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
    let mut env = topocsa::new(&mut store, &mut tt.connections, &tt.stations, tt.cut, 0, 0.01, true);

    let o = 100;
    let d = 1000;
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
    gtfs::load("./tests/fixtures/gtfs_minimal_swiss/", chrono::NaiveDate::from_ymd_opt(2024, 1, 1).unwrap(), chrono::NaiveDate::from_ymd_opt(2024, 1, 10).unwrap(), &mut stations, &mut routes, &mut connections);

    let mut env = topocsa::prepare(&mut store, &mut connections, &stations, 0, 0.1, false);

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

