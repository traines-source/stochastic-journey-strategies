extern crate rmp_serde as rmps;

use std::collections::HashMap;
use rustc_hash::FxHashSet;
use std::env;
use serde::Serialize;
use rmps::Serializer;
use std::io::Write;
use std::fs;
use stost::{distribution_store, walking};
use stost::query::{topocsa, Query};
use stost::gtfs;
use std::time::Instant;
use stost::query::Queriable;

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
}

#[test]
#[ignore]
fn create_simulation_samples() {
    let samples = gtfs::create_simulation_samples(GTFS_PATH, day(2023, 11, 2), day(2023, 11, 9), None);
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
fn create_bw_simulation_samples() {
    let samples = gtfs::create_simulation_samples("../gtfs/german-gtfs/2023-10-30/", day(2023, 11, 2), day(2023, 11, 3), Some((47.525, 7.493, 49.774, 10.514)));
    let buf = serde_json::to_vec(&samples).unwrap();
    let mut file = fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open("./simulation/samples/bw.json").expect("file not openable");
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
    let q = Query {
        origin_idx: 10000,
        destination_idx: 20000,
        start_time: 8100,
        max_time: 8820
    };
    
    println!("querying...");
    let station_labels = env.query(q);
    let origin_deps = &station_labels[q.origin_idx];
    let best_conn = origin_deps.last().unwrap();
    let second_best_conn = &origin_deps[origin_deps.len()/3];
    println!("{:?} {:?} {:?} {:?} {:?}{:?}", tt.stations[q.origin_idx].name, tt.stations[q.destination_idx].name, &tt.connections[tt.order[best_conn.connection_id]].departure, best_conn.destination_arrival, &tt.connections[tt.order[second_best_conn.connection_id]].departure, second_best_conn.destination_arrival);
}

#[test]
#[ignore]
fn gtfs_with_contr() {
    let mut store = distribution_store::Store::new();
    store.load_distributions("./data/ch_sbb.csv");

    let mut tt = gtfs::load_gtfs_cache(CACHE_PATH);
    //gtfs::shorten_footpaths(&mut tt.stations);
    let mut env = topocsa::Environment::new(&mut store, &mut tt.connections, &tt.stations, &mut tt.cut, &mut tt.order, 0, 0.0, 0.0, true, false);
    let contr = gtfs::get_station_contraction(&tt.stations);
    env.set_station_contraction(&contr);
    let q = Query {
        origin_idx: 27224,
        destination_idx: 2645,
        start_time: 7800,
        max_time: 7800+720
    };
    println!("querying...");
    let station_labels = env.query(q);
    let origin_deps = &station_labels[contr.stop_to_group[q.origin_idx]];
    let best_conn = origin_deps.iter().rev().filter(|l| tt.connections[tt.order[l.connection_id]].departure.projected() > 7800).take(1).last().unwrap();
    let second_best_conn = &origin_deps[origin_deps.len()/3];
    //println!("{:?}", contr);

    println!("{:?} {:?} {:?} {:?} {:?} {:?}{:?}", tt.stations[q.origin_idx].name, tt.stations[q.destination_idx].name, &tt.connections[tt.order[best_conn.connection_id]].departure, best_conn.destination_arrival, best_conn.destination_arrival.mean(), &tt.connections[tt.order[second_best_conn.connection_id]].departure, second_best_conn.destination_arrival);
}


#[test]
#[ignore]
fn gtfs_with_extended_walking() {
    let mut store = distribution_store::Store::new();
    store.load_distributions("./data/ch_sbb.csv");

    let mut tt = gtfs::load_gtfs_cache(CACHE_PATH);

    let contr = gtfs::get_station_contraction(&tt.stations);
    let q = Query {
        origin_idx: 40209,
        destination_idx: 39166,
        start_time: 7800,
        max_time: 7800+720
    };
    let rtree = walking::init_rtree(&tt.stations);
    println!("querying...");
    let (_walking_tt, walking_origin_idx, _walking_destination_idx, station_labels) = walking::query_with_extended_walking(&mut store, &mut tt, q, 7200, &contr, &rtree);
    let origin_deps = &station_labels[contr.stop_to_group[walking_origin_idx]];
    let best_conn = origin_deps.last().unwrap();
    let second_best_conn = &origin_deps[origin_deps.len()/3];
    //println!("{:?}", contr);

    println!("{:?} {:?} {:?} {:?} {:?} {:?}{:?}", tt.stations[q.origin_idx].name, tt.stations[q.destination_idx].name, &tt.connections[tt.order[best_conn.connection_id]].departure, best_conn.destination_arrival, best_conn.destination_arrival.mean(), &tt.connections[tt.order[second_best_conn.connection_id]].departure, second_best_conn.destination_arrival);
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

    let q = Query {
        origin_idx: 24868,
        destination_idx: 33777,
        start_time: 7200,
        max_time: 8640
    };
    println!("querying rel...");
    let sl = env.query(q);

    let start_ts = Instant::now();
    let relevant_stations = env.relevant_stations(q, &sl);
    println!("elapsed relevant stations: {}", start_ts.elapsed().as_millis());
    let connection_pairs = env.relevant_connection_pairs(q, &relevant_stations, 1000);
    println!("elapsed incl relevant connections: {} len: {}", start_ts.elapsed().as_millis(), connection_pairs.len());
    env.preprocess();
    let station_labels = env.pair_query(q, &connection_pairs);
    let origin_deps = &station_labels[contr.stop_to_group[q.origin_idx]];
    let best_conn = origin_deps.last().unwrap();
    println!("mean: {}, {}", best_conn.destination_arrival.mean, best_conn.destination_arrival.mean());
    let second_best_conn = &origin_deps[origin_deps.len()/3];
    //println!("{:?}", contr);

    println!("{:?} {:?} {:?} {:?} {:?}{:?}", tt.stations[q.origin_idx].name, tt.stations[q.destination_idx].name, &tt.connections[tt.order[best_conn.connection_id]].departure, best_conn.destination_arrival, &tt.connections[tt.order[second_best_conn.connection_id]].departure, second_best_conn.destination_arrival);
}

#[test]
#[ignore]
fn gtfs_with_rt() {
    let mut store = distribution_store::Store::new();
    store.load_distributions("./data/ch_sbb.csv");

    let mut tt = gtfs::load_gtfs_cache(CACHE_PATH);
    let t = gtfs::load_timetable(GTFS_PATH, day(2023, 11, 2), day(2023, 11, 3));
    let mut env = topocsa::Environment::new(&mut store, &mut tt.connections, &tt.stations, &mut tt.cut, &mut tt.order, 7850, 0.01, 0.0, true, false);
    let path = format!("{}2023-11-02T07:00:03+01:00.gtfsrt", GTFSRT_PATH);
    gtfs::load_realtime(&path, &t, &tt.transport_and_day_to_connection_id,
        |connection_id: usize, is_departure: bool, location_idx: Option<usize>, in_out_allowed: Option<bool>, delay: Option<i16>| {
            env.update(connection_id, is_departure, location_idx, in_out_allowed, delay)
        }
    );
    let path = format!("{}2023-11-02T11:50:03+01:00.gtfsrt", GTFSRT_PATH);
    gtfs::load_realtime(&path, &t, &tt.transport_and_day_to_connection_id,
        |connection_id: usize, is_departure: bool, location_idx: Option<usize>, in_out_allowed: Option<bool>, delay: Option<i16>| {
            env.update(connection_id, is_departure, location_idx, in_out_allowed, delay)
        }
    );
    let contr = gtfs::get_station_contraction(&tt.stations);
    env.set_station_contraction(&contr);
    
    let q = Query {
        origin_idx: 27224,
        destination_idx: 2645,
        start_time: 7800,
        max_time: 7800+720
    };
    println!("querying...");
    let station_labels = env.query(q);
    let origin_deps = &station_labels[contr.stop_to_group[25835]];
    let conns: Vec<&stost::query::ConnectionLabel> = origin_deps.iter().rev().filter(|l| tt.connections[tt.order[l.connection_id]].departure.projected() > 7848).take(2).collect();
    let best_conn = conns[0];
    let second_best_conn = conns[1];
    println!("{:?} {:?} {:?} {:?} {:?}{:?}", tt.stations[q.origin_idx].name, tt.stations[q.destination_idx].name, &tt.connections[tt.order[best_conn.connection_id]].departure, best_conn.destination_arrival, &tt.connections[tt.order[second_best_conn.connection_id]].departure, second_best_conn.destination_arrival);
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

    let q = Query {
        origin_idx: 10000,
        destination_idx: 20000,
        start_time: 7800,
        max_time: 8220
    };
    println!("querying...");
    let station_labels = env.query(q);
    let origin_deps = &station_labels[q.origin_idx];
    let best_conn = origin_deps.last().unwrap();
    let second_best_conn = &origin_deps[origin_deps.len()/3];
    println!("{:?} {:?} {:?} {:?} {:?}{:?}", tt.stations[q.origin_idx].name, tt.stations[q.destination_idx].name, &tt.connections[tt.order[best_conn.connection_id]].departure, best_conn.destination_arrival, &tt.connections[tt.order[second_best_conn.connection_id]].departure, second_best_conn.destination_arrival);

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

    let q = Query {
        origin_idx: 11,
        destination_idx: 69,
        start_time: 7200,
        max_time: 8640
    };
    println!("{:?} {:?} {:?} {:?}", stations[q.origin_idx].id, stations[q.origin_idx].name, stations[q.destination_idx].id, stations[q.destination_idx].name);
    assert_eq!(stations[q.destination_idx].footpaths.len(), 3);
    
    let station_labels = env.query(q);
    let origin_deps = &station_labels[q.origin_idx];
    let best_conn = origin_deps.last().unwrap();
    let second_best_conn = &origin_deps[origin_deps.len()/3];
    println!("{:?} {:?} {:?} {:?} {:?}{:?}", stations[q.origin_idx].name, stations[q.destination_idx].name, &connections[order[best_conn.connection_id]].departure, best_conn.destination_arrival, &connections[order[second_best_conn.connection_id]].departure, second_best_conn.destination_arrival);
    
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
        let q = Query {
            origin_idx: 10000,
            destination_idx: 20000,
            start_time: 7200,
            max_time: (7200+i*1440) as i32
        };    
        let start_ts = Instant::now();
        env.preprocess();
        let prepr = start_ts.elapsed().as_millis();
        let mem = memory_stats::memory_stats().unwrap().physical_mem;
        let start_ts = Instant::now();
        let station_labels = env.query(q);
        let query = start_ts.elapsed().as_millis();
        stats.push((i, prepr, query, mem));
        let origin_deps = &station_labels[contr.stop_to_group[q.origin_idx]];
        let best_conn = origin_deps.last().unwrap();
        println!("STATS: {:?} {:?}", best_conn.destination_arrival.mean(), stats);
    }

}