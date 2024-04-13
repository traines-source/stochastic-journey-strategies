use std::collections::HashMap;
use rustc_hash::FxHashSet;
use stost::distribution_store;
use stost::query::csameat;
use stost::query::Queriable;
use stost::gtfs;
use stost::query::Query;

const CACHE_PATH: &str = "./tests/fixtures/timetable.ign.cache";
const GTFS_PATH: &str = "../gtfs/swiss-gtfs/2023-11-06/";
const GTFSRT_PATH: &str = "../gtfs/swiss-gtfs-rt/2023-11-02/";

fn day(year: i32, month: u32, day: u32) -> chrono::NaiveDate {
    chrono::NaiveDate::from_ymd_opt(year, month, day).unwrap()
}

#[test]
#[ignore]
fn gtfs_with_contr() {
    let mut store = distribution_store::Store::new();
    store.load_distributions("./data/ch_sbb.csv");
    store.nonnegative();

    let mut tt = gtfs::load_gtfs_cache(CACHE_PATH);
    //gtfs::shorten_footpaths(&mut tt.stations);
    let mut env = csameat::Environment::new(&mut store, &mut tt.connections, &tt.stations, &mut tt.cut, &mut tt.order, 0);
    let contr = gtfs::get_station_contraction(&tt.stations);
    env.set_station_contraction(&contr);
    let q = Query {
        origin_idx: 10000,
        destination_idx: 20000,
        start_time: 7200,
        max_time: 8640
    };
    env.preprocess();
    println!("querying...");
    let station_labels = env.full_query(q.origin_idx, q.destination_idx, 7200, 8640);
    let decision_graph = env.get_decision_graph(q.origin_idx, q.destination_idx, 7200, &station_labels);
    let dummy = HashMap::new();
    let connection_pairs = env.relevant_connection_pairs(q, &dummy, 0);
    let origin_deps = &station_labels[contr.stop_to_group[q.origin_idx]];
    let best_conn = origin_deps.last().unwrap();
    let second_best_conn = &origin_deps[origin_deps.len()/3];
    //println!("{:?}", contr);

    println!("{:?} {:?} {:?} {:?} {:?}{:?}", tt.stations[q.origin_idx].name, tt.stations[q.destination_idx].name, &tt.connections[tt.order[best_conn.connection_id]].departure, best_conn.destination_arrival, &tt.connections[tt.order[best_conn.connection_id]].departure, second_best_conn.destination_arrival);

    let origin_deps = &decision_graph[contr.stop_to_group[q.origin_idx]];
    let best_conn = origin_deps.last().unwrap();
    let cpreverse: HashMap<usize, usize> = connection_pairs.iter().map(|(arr,dep)| (*dep as usize, *arr as usize)).collect();
    let arr_conn = &tt.connections[tt.order[cpreverse[&tt.connections[tt.order[best_conn.connection_id]].id]]];
    println!("connpairs: {:?}", connection_pairs.len());

    println!("depconn: {:?} arrconn: {:?}", &tt.connections[tt.order[best_conn.connection_id]], &arr_conn);

}

#[test]
#[ignore]
fn gtfs_with_rt() {
    let mut store = distribution_store::Store::new();
    store.load_distributions("./data/ch_sbb.csv");
    store.nonnegative();

    let mut tt = gtfs::GtfsTimetable {
        stations: vec![],
        connections: vec![],
        cut: FxHashSet::default(),
        order: vec![],
        transport_and_day_to_connection_id: HashMap::new()
    };
    let mut routes = vec![];
    let t = gtfs::load_timetable(GTFS_PATH, day(2023, 11, 2), day(2023, 11, 3));
    let mapping = gtfs::retrieve(&t, &mut tt.stations, &mut routes, &mut tt.connections);
    let mut env = csameat::Environment::new(&mut store, &mut tt.connections, &tt.stations, &mut tt.cut, &mut tt.order, 7500);
    let path = format!("{}2023-11-02T07:00:03+01:00.gtfsrt", GTFSRT_PATH);
    gtfs::load_realtime(&path, &t, &mapping,
        |connection_id: usize, is_departure: bool, location_idx: Option<usize>, in_out_allowed: Option<bool>, delay: Option<i16>| {
            env.update(connection_id, is_departure, location_idx, in_out_allowed, delay)
        }
    );
    let contr = gtfs::get_station_contraction(&tt.stations);
    env.set_station_contraction(&contr);
    let q = Query {
        origin_idx: 38895,
        destination_idx: 34984,
        start_time: 7500,
        max_time: 8220
    };
    println!("{:?} {:?}", tt.stations[q.origin_idx].name, tt.stations[q.destination_idx].name);
    env.preprocess();
    println!("querying...");
    let station_labels = env.full_query(q.origin_idx, q.destination_idx, 7500, 8220);
    let decision_graph = env.get_decision_graph(q.origin_idx, q.destination_idx, 7500, &station_labels);
    let dummy = HashMap::new();
    let connection_pairs_reverse = env.relevant_connection_pairs(q, &dummy, 0);
    let origin_deps = &station_labels[contr.stop_to_group[q.origin_idx]];
    let best_conn = origin_deps.last().unwrap();
    let second_best_conn = &origin_deps[origin_deps.len()/3];
    //println!("{:?}", contr);

    println!("{:?} {:?} {:?}{:?}", &tt.connections[tt.order[best_conn.connection_id]].departure, best_conn.destination_arrival, &tt.connections[tt.order[best_conn.connection_id]].departure, second_best_conn.destination_arrival);

    let origin_deps = &decision_graph[contr.stop_to_group[q.origin_idx]];
    let best_conn = origin_deps.last().unwrap();
    let cpreverse: HashMap<usize, usize> = connection_pairs_reverse.iter().map(|(arr,dep)| (*arr as usize, *dep as usize)).collect();
    let arr_conn = &tt.connections[tt.order[cpreverse[&best_conn.connection_id]]];
    println!("connpairs: {:?}", connection_pairs_reverse.len());

    println!("depconn: {:?} arrconn: {:?}", &tt.connections[tt.order[best_conn.connection_id]], &arr_conn);

}

