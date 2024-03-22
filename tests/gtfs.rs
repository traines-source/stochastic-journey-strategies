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
use stost::query::Query;

const CACHE_PATH: &str = "./tests/fixtures/timetable.ign.cache";
const GTFS_PATH: &str = "../gtfs/swiss-gtfs/2023-11-06/";
const GTFSRT_PATH: &str = "../gtfs/swiss-gtfs-rt/2023-11-02/";

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
    topocsa::prepare(&mut store, &mut tt.connections, &tt.stations, &mut tt.cut, &mut tt.order, 8000, 0.01, true);
    println!("elapsed: {}", start_ts.elapsed().as_millis());
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
    let mut env = topocsa::Environment::new(&mut store, &mut tt.connections, &tt.stations, &mut tt.cut, &mut tt.order, 8100, 0.01, 0.01, true, false);
    //dbg!(&tt.stations[9032], &tt.stations[34734]);
    let o = 10000;
    let d = 20000;
    println!("querying...");
    let station_labels = env.query(o, d, 8100, 8820);
    let origin_deps = &station_labels[o];
    let best_conn = origin_deps.last().unwrap();
    let second_best_conn = &origin_deps[origin_deps.len()/3];
    println!("{:?} {:?} {:?} {:?} {:?}{:?}", tt.stations[o].name, tt.stations[d].name, &tt.connections[tt.order[best_conn.connection_id]].departure, best_conn.destination_arrival, &tt.connections[tt.order[second_best_conn.connection_id]].departure, second_best_conn.destination_arrival);
}

#[test]
#[ignore]
fn gtfs_with_contr() {
    let mut store = distribution_store::Store::new();
    store.load_distributions("./data/ch_sbb.csv");

    let mut tt = gtfs::load_gtfs_cache(CACHE_PATH);
    //gtfs::shorten_footpaths(&mut tt.stations);
    let mut env = topocsa::Environment::new(&mut store, &mut tt.connections, &tt.stations, &mut tt.cut, &mut tt.order, 0, 0.01, 0.001, true, false);
    let contr = gtfs::get_station_contraction(&tt.stations);
    env.set_station_contraction(&contr);
    let o = 10000;
    let d = 20000;
    println!("querying...");
    let station_labels = env.query(o, d, 7200, 8640);
    let origin_deps = &station_labels[contr.stop_to_group[o]];
    let best_conn = origin_deps.last().unwrap();
    let second_best_conn = &origin_deps[origin_deps.len()/3];
    //println!("{:?}", contr);

    println!("{:?} {:?} {:?} {:?} {:?} {:?}{:?}", tt.stations[o].name, tt.stations[d].name, &tt.connections[tt.order[best_conn.connection_id]].departure, best_conn.destination_arrival, best_conn.destination_arrival.mean(), &tt.connections[tt.order[second_best_conn.connection_id]].departure, second_best_conn.destination_arrival);
}

#[test]
#[ignore]
fn gtfs_with_relevant_stations() {
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
    let t = gtfs::load_timetable(GTFS_PATH, day(2023, 11, 2), day(2023, 11, 3));
    tt.transport_and_day_to_connection_id = gtfs::retrieve(&t, &mut tt.stations, &mut routes, &mut tt.connections);
    let mut env = topocsa::Environment::new(&mut store, &mut tt.connections, &tt.stations, &mut tt.cut, &mut tt.order, 7200, 0.01, 0.001, true, false);
    let contr = gtfs::get_station_contraction(&tt.stations);
    env.set_station_contraction(&contr);
    env.preprocess();
    let path = format!("{}2023-11-02T07:00:03+01:00.gtfsrt", GTFSRT_PATH);
    gtfs::load_realtime(&path, &t, &tt.transport_and_day_to_connection_id,
        |connection_id: usize, is_departure: bool, location_idx: Option<usize>, in_out_allowed: Option<bool>, delay: Option<i16>| {
            env.update(connection_id, is_departure, location_idx, in_out_allowed, delay)
        }
    );

    let o = 24868;
    let d = 33777;
    println!("querying rel...");
    let start_time = 7200;
    let max_time = 8640;
    let sl = env.query(o, d, start_time, max_time);

    let start_ts = Instant::now();
    let relevant_stations = env.relevant_stations(o, d, &sl);
    println!("elapsed relevant stations: {}", start_ts.elapsed().as_millis());
    let connection_pairs = env.relevant_connection_pairs(&relevant_stations);
    println!("elapsed incl relevant connections: {} len: {}", start_ts.elapsed().as_millis(), connection_pairs.len());
    env.preprocess();
    let station_labels = env.pair_query(o, d, start_time, max_time, &connection_pairs);
    let origin_deps = &station_labels[contr.stop_to_group[o]];
    let best_conn = origin_deps.last().unwrap();
    println!("mean: {}, {}", best_conn.destination_arrival.mean, best_conn.destination_arrival.mean());
    let second_best_conn = &origin_deps[origin_deps.len()/3];
    //println!("{:?}", contr);

    println!("{:?} {:?} {:?} {:?} {:?}{:?}", tt.stations[o].name, tt.stations[d].name, &tt.connections[tt.order[best_conn.connection_id]].departure, best_conn.destination_arrival, &tt.connections[tt.order[second_best_conn.connection_id]].departure, second_best_conn.destination_arrival);
}

#[test]
#[ignore]
fn gtfs_with_rt() {
    let mut store = distribution_store::Store::new();
    store.load_distributions("./data/ch_sbb.csv");

    let mut tt = gtfs::load_gtfs_cache(CACHE_PATH);
    let t = gtfs::load_timetable(GTFS_PATH, day(2023, 11, 2), day(2023, 11, 3));
    let mut env = topocsa::Environment::new(&mut store, &mut tt.connections, &tt.stations, &mut tt.cut, &mut tt.order, 7500, 0.01, 0.01, true, false);
    let path = format!("{}2023-11-02T07:00:03+01:00.gtfsrt", GTFSRT_PATH);
    gtfs::load_realtime(&path, &t, &tt.transport_and_day_to_connection_id,
        |connection_id: usize, is_departure: bool, location_idx: Option<usize>, in_out_allowed: Option<bool>, delay: Option<i16>| {
            env.update(connection_id, is_departure, location_idx, in_out_allowed, delay)
        }
    );
    let path = format!("{}2023-11-02T10:00:03+01:00.gtfsrt", GTFSRT_PATH);
    gtfs::load_realtime(&path, &t, &tt.transport_and_day_to_connection_id,
        |connection_id: usize, is_departure: bool, location_idx: Option<usize>, in_out_allowed: Option<bool>, delay: Option<i16>| {
            env.update(connection_id, is_departure, location_idx, in_out_allowed, delay)
        }
    );
    let contr = gtfs::get_station_contraction(&tt.stations);
    env.set_station_contraction(&contr);
    let o = 10100;
    let d = 20100;
    println!("querying...");
    let station_labels = env.query(o, d, 7500, 8220);
    let origin_deps = &station_labels[o];
    let best_conn = origin_deps.last().unwrap();
    let second_best_conn = &origin_deps[origin_deps.len()/3];
    println!("{:?} {:?} {:?} {:?} {:?}{:?}", tt.stations[o].name, tt.stations[d].name, &tt.connections[tt.order[best_conn.connection_id]].departure, best_conn.destination_arrival, &tt.connections[tt.order[second_best_conn.connection_id]].departure, second_best_conn.destination_arrival);
}

#[test]
#[ignore]
fn load_only_gtfs_with_rt() {
    let mut store = distribution_store::Store::new();
    store.load_distributions("./data/ch_sbb.csv");

    let mut tt = gtfs::GtfsTimetable::new();
    let mut routes = vec![];
    let t = gtfs::load_timetable("/gtfs/swiss-gtfs/2024-01-15/", day(2024, 1, 15), day(2024, 1, 16));
    let mapping = gtfs::retrieve(&t, &mut tt.stations, &mut routes, &mut tt.connections);    
    let mut env = topocsa::Environment::new(&mut store, &mut tt.connections, &tt.stations, &mut tt.cut, &mut tt.order, 7800, 0.01, 0.01, true, false);
    let path = "/gtfs/swiss-gtfs-rt/2024-01-15/2024-01-15T10:14:03+01:00.gtfsrt";
    gtfs::load_realtime(&path, &t, &mapping,
        |connection_id: usize, is_departure: bool, location_idx: Option<usize>, in_out_allowed: Option<bool>, delay: Option<i16>| {
            env.update(connection_id, is_departure, location_idx, in_out_allowed, delay)
        }
    );
    env.preprocess();
    let path = "/gtfs/swiss-gtfs-rt/2024-01-15/2024-01-15T15:38:03+01:00.gtfsrt";
    gtfs::load_realtime(&path, &t, &mapping,
        |connection_id: usize, is_departure: bool, location_idx: Option<usize>, in_out_allowed: Option<bool>, delay: Option<i16>| {
            env.update(connection_id, is_departure, location_idx, in_out_allowed, delay)
        }
    );
    env.preprocess();
    let contr = gtfs::get_station_contraction(&tt.stations);
    env.set_station_contraction(&contr);

    let o = 10000;
    let d = 20000;
    println!("querying...");
    let station_labels = env.query(o, d, 7800, 8220);
    let origin_deps = &station_labels[o];
    let best_conn = origin_deps.last().unwrap();
    let second_best_conn = &origin_deps[origin_deps.len()/3];
    println!("{:?} {:?} {:?} {:?} {:?}{:?}", tt.stations[o].name, tt.stations[d].name, &tt.connections[tt.order[best_conn.connection_id]].departure, best_conn.destination_arrival, &tt.connections[tt.order[second_best_conn.connection_id]].departure, second_best_conn.destination_arrival);

    for i in 0..tt.connections.len() {
        assert_eq!(tt.connections[tt.order[i]].id, i); 
    }
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
    let mut cut = FxHashSet::default();

    let mut env = topocsa::prepare(&mut store, &mut connections, &stations, &mut cut, &mut order, 0, 0.01, false);
    gtfs::load_realtime("./tests/fixtures/2024-01-02T01_48_02+01_00.gtfsrt", &t, &map, 
        |connection_id: usize, is_departure: bool, location_idx: Option<usize>, in_out_allowed: Option<bool>, delay: Option<i16>| {
            env.update(connection_id, is_departure, location_idx, in_out_allowed, delay)
        }
    );

    let o = 11;
    let d = 69;
    println!("{:?} {:?} {:?} {:?}", stations[o].id, stations[o].name, stations[d].id, stations[d].name);
    assert_eq!(stations[d].footpaths.len(), 3);

    let station_labels = env.query(o, d, 7200, 8640);
    let origin_deps = &station_labels[o];
    let best_conn = origin_deps.last().unwrap();
    let second_best_conn = &origin_deps[origin_deps.len()/3];
    println!("{:?} {:?} {:?} {:?} {:?}{:?}", stations[o].name, stations[d].name, &connections[order[best_conn.connection_id]].departure, best_conn.destination_arrival, &connections[order[second_best_conn.connection_id]].departure, second_best_conn.destination_arrival);
    
}




#[test]
#[ignore]
fn gtfs_repeated() {
    let mut store = distribution_store::Store::new();
    store.load_distributions("./data/ch_sbb.csv");

    let mut stats = vec![];

    for i in vec![1, 10, 20, 30, 40, 50] {
        let mut tt = gtfs::GtfsTimetable::new();
        let mut routes = vec![];
        let t = gtfs::load_timetable(GTFS_PATH, day(2023, 11, 1), day(2023, 11+i/30, 1+i%30));
        gtfs::retrieve(&t, &mut tt.stations, &mut routes, &mut tt.connections);    
        let mut env = topocsa::Environment::new(&mut store, &mut tt.connections, &tt.stations, &mut tt.cut, &mut tt.order, 0, 0.01, 0.01, true, true);
        let contr = gtfs::get_station_contraction(&tt.stations);
        env.set_station_contraction(&contr);
        let o = 10000;
        let d = 20000;
        let start_ts = Instant::now();
        env.preprocess();
        let prepr = start_ts.elapsed().as_millis();
        let mem = memory_stats::memory_stats().unwrap().physical_mem;
        let start_ts = Instant::now();
        let station_labels = env.query(o, d, 7200, (7200+i*1440) as i32);
        let query = start_ts.elapsed().as_millis();
        stats.push((i, prepr, query, mem));
        let origin_deps = &station_labels[contr.stop_to_group[o]];
        let best_conn = origin_deps.last().unwrap();
        println!("STATS: {:?} {:?}", best_conn.destination_arrival.mean(), stats);
    }

}