#[macro_use]
extern crate assert_float_eq;


use stost::distribution_store;
use stost::query::topocsa;
use stost::gtfs;


#[test]
#[ignore]
fn gtfs() {
    let mut store = distribution_store::Store::new();
    store.load_distributions("./data/ch_sbb.csv");

    let mut stations = vec![];
    let mut routes = vec![];
    let mut connections = vec![];
    gtfs::load("/gtfs/swiss-gtfs/2023-11-06/", chrono::NaiveDate::from_ymd_opt(2023, 11, 1).unwrap(), chrono::NaiveDate::from_ymd_opt(2023, 11, 2).unwrap(), &mut stations, &mut routes, &mut connections);
    let mut env = topocsa::prepare(&mut store, &mut connections, &stations, 0, 0.1);

    let o = 100;
    let d = 1000;
    let station_labels = env.query(&stations[o], &stations[d]);
    let origin_deps = &station_labels[&o];
    let best_conn = &connections[*origin_deps.last().unwrap()];
    let second_best_conn = &connections[origin_deps[origin_deps.len()-2]];
    println!("{:?} {:?} {:?} {:?} {:?}{:?}", stations[o].name, stations[d].name, best_conn.departure, best_conn.destination_arrival, second_best_conn.departure, second_best_conn.destination_arrival);
    
}

#[test]
fn gtfs_small() {
    let mut store = distribution_store::Store::new();
    store.load_distributions("./data/ch_sbb.csv");

    let mut stations = vec![];
    let mut routes = vec![];
    let mut connections = vec![];
    gtfs::load("./tests/fixtures/gtfs_minimal_swiss/", chrono::NaiveDate::from_ymd_opt(2024, 1, 1).unwrap(), chrono::NaiveDate::from_ymd_opt(2024, 1, 10).unwrap(), &mut stations, &mut routes, &mut connections);

    let mut env = topocsa::prepare(&mut store, &mut connections, &stations, 0, 0.1);

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

