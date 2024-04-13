use chrono::Days;
use rouille::Response;
use rstar::RTree;
use serde::Deserialize;
use stost::query::Query;
use std::collections::HashMap;
use std::env;
use std::io::Read;
use std::sync::Mutex;
use stost::connection;
use stost::connection::Route;
use stost::distribution_store;
use stost::distribution_store::Store;
use stost::gtfs;
use stost::gtfs::GtfsTimetable;
use stost::gtfs::StationContraction;
use stost::query;
use stost::query::topocsa;
use stost::query::Queriable;
use stost::walking;
use stost::walking::StationLocation;
use stost::wire::serde::to_mtime;
use stost::wire::serde::QueryMetadata;

#[derive(Deserialize)]
struct ApiConfig {
    systems: HashMap<String, ApiSystem>,
}

#[allow(dead_code)]
#[derive(Deserialize)]
struct ApiSystem {
    provide_timetable: bool,
    provide_relevant_stations: bool,
    gtfs_glob: String,
    gtfsrt_glob: String,
    #[serde(skip_deserializing)]
    store: Option<Store>,
    //#[serde(skip_deserializing)]
    //t: Option<Timetable>,
    #[serde(skip_deserializing)]
    tt: Option<GtfsTimetable>,
    #[serde(skip_deserializing)]
    station_idx: HashMap<String, usize>,
    #[serde(skip_deserializing)]
    routes: Vec<Route>,
    #[serde(skip_deserializing)]
    rtree: RTree<StationLocation>,
    #[serde(skip_deserializing)]
    contraction: Option<StationContraction>,
    #[serde(skip_deserializing)]
    reference_ts: i64,
}

fn load_config() -> ApiConfig {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        panic!("Usage: api CONFIG_FILE");
    }
    let buf = std::fs::read(&args[1]).unwrap();
    serde_json::from_slice(&buf).unwrap()
}

fn get_last_glob_path(glob: &str) -> String {
    let file = glob::glob(glob)
        .expect("Failed to read glob pattern")
        .last()
        .expect("no eligible file");
    let f = file.as_ref().unwrap().to_str().unwrap().to_owned();
    println!("Loading {}", f);
    f
}

fn prepare_configured_systems(config: &mut ApiConfig) {
    for c in config.systems.iter_mut() {
        let mut store = distribution_store::Store::new();
        store.load_distributions(&format!("./data/{}.csv", c.0));
        if c.1.provide_timetable {
            let now = chrono::offset::Local::now().date_naive(); //chrono::NaiveDate::from_ymd_opt(2023, 11, 2).unwrap();
            let path = get_last_glob_path(&c.1.gtfs_glob);
            let t = gtfs::load_timetable(
                &path,
                now,
                now.checked_add_days(Days::new(1)).unwrap(),
            );
            println!("start_ts: {}", t.get_start_day_ts());
            let mut tt = gtfs::GtfsTimetable::new();
            tt.transport_and_day_to_connection_id =
                gtfs::retrieve(&t, &mut tt.stations, &mut c.1.routes, &mut tt.connections);
            walking::shorten_footpaths(&mut tt.stations);
            c.1.contraction = Some(gtfs::get_station_contraction(&mut tt.stations));
            c.1.station_idx = tt
                .stations
                .iter()
                .enumerate()
                .map(|s| (s.1.id.clone(), s.0))
                .collect();
            c.1.reference_ts = t.get_start_day_ts();
            let mut env = topocsa::Environment::new(
                &mut store,
                &mut tt.connections,
                &tt.stations,
                &mut tt.cut,
                &mut tt.order,
                0,
                0.01,
                0.001,
                true,
                true,
            );
            let path = get_last_glob_path(&c.1.gtfsrt_glob);
            gtfs::load_realtime(
                &path,
                &t,
                &tt.transport_and_day_to_connection_id,
                |connection_id: usize,
                 is_departure: bool,
                 location_idx: Option<usize>,
                 in_out_allowed: Option<bool>,
                 delay: Option<i16>| {
                    env.update(
                        connection_id,
                        is_departure,
                        location_idx,
                        in_out_allowed,
                        delay,
                    )
                },
            );
            c.1.rtree = walking::init_rtree(&tt.stations);
            //c.1.t = Some(t);
            c.1.tt = Some(tt);
        }
        c.1.store = Some(store);
    }
}

fn query_on_timetable(system_conf: &mut ApiSystem, mut metadata: QueryMetadata) -> Vec<u8> {
    let query_window = 720;
    let tt = system_conf.tt.as_mut().unwrap();
    let now = to_mtime(metadata.now, system_conf.reference_ts);
    let start_time = to_mtime(metadata.start_ts, system_conf.reference_ts);
    let full_query = Query {
        origin_idx: system_conf.station_idx[&metadata.origin_id],
        destination_idx: system_conf.station_idx[&metadata.destination_id],
        start_time: start_time,
        max_time: start_time+query_window
    };
    let (mut walking_tt, walking_origin_idx, walking_destination_idx, walking_station_labels) = walking::query_with_extended_walking(
        system_conf.store.as_mut().unwrap(),
        tt,
        full_query,
        now,
        system_conf.contraction.as_ref().unwrap(),
        &system_conf.rtree,
    );
    if walking_tt.stations.is_empty() {
        return vec![];
    }
    let mut rel_env = topocsa::Environment::new(
        system_conf.store.as_mut().unwrap(),
        &mut walking_tt.connections,
        &walking_tt.stations,
        &mut walking_tt.cut,
        &mut walking_tt.order,
        now,
        0.01,
        0.001,
        true,
        false,
    );
    let walking_query = Query {
        origin_idx: walking_origin_idx,
        destination_idx: walking_destination_idx,
        start_time: full_query.start_time,
        max_time: full_query.max_time
    };
    let weights_by_station_idx =
        rel_env.get_relevant_stations(walking_query.origin_idx, walking_query.destination_idx, &walking_station_labels, false);
    let relevant_connection_pairs = rel_env.relevant_connection_pairs(walking_query, &weights_by_station_idx, 10000);
    println!("rel. conns: {}", relevant_connection_pairs.len());
    let no_extended_walking = HashMap::new();
    let relevant_timetable = walking::create_relevant_timetable_with_extended_walking(
        &mut walking_tt.connections,
        &walking_tt.stations,
        &walking_tt.order,
        relevant_connection_pairs,
        &no_extended_walking,
        walking_origin_idx, walking_destination_idx
    );
    println!("conns: {} stops: {}", relevant_timetable.0.connections.len(), relevant_timetable.0.stations.len());
    println!("fromto: {} {}", relevant_timetable.1, relevant_timetable.2);
    metadata.origin_idx = relevant_timetable.1;
    metadata.destination_idx = relevant_timetable.2;
    metadata.start_ts = system_conf.reference_ts;
    stost::wire::serde::serialize_protobuf(
        &relevant_timetable.0.stations,
        &system_conf.routes,
        &relevant_timetable.0.connections,
        system_conf.contraction.as_ref(),
        &metadata
    )
}

fn query_on_given(
    system_conf: &mut ApiSystem,
    input_stations: &mut Vec<connection::Station>,
    input_routes: &Vec<connection::Route>,
    input_connections: &mut Vec<connection::Connection>,
    metadata: QueryMetadata,
) -> Vec<u8> {
    walking::create_quadratic_footpaths(input_stations);
    println!("querying...");
    query::query(
        system_conf.store.as_mut().unwrap(),
        input_connections,
        &input_stations,
        metadata.origin_idx,
        metadata.destination_idx,
        0,
        1440 * 2,
        to_mtime(metadata.now, metadata.start_ts),
    );
    stost::wire::serde::serialize_protobuf(
        &input_stations,
        &input_routes,
        &input_connections,
        system_conf.contraction.as_ref(),
        &metadata,
    )
}

fn main() {
    println!("starting...");
    let mut conf = load_config();
    prepare_configured_systems(&mut conf);
    let conf_mutex = Mutex::new(conf);

    rouille::start_server("0.0.0.0:1234", move |request| {
        println!("receiving req...");

        //let bytes: Vec<u8> = serde::read_protobuf("./tests/fixtures/basic.pb");
        let mut bytes: Vec<u8> = vec![];
        let result = request.data().unwrap().read_to_end(&mut bytes);
        if result.is_err() {
            panic!("{:?}", result);
        }
        //serde::write_protobuf(&bytes, "./basic.pb");
        let mut input_stations: Vec<connection::Station> = vec![];
        let mut input_routes = vec![];
        let mut input_connections = vec![];
        let metadata = stost::wire::serde::deserialize_protobuf(
            bytes,
            &mut input_stations,
            &mut input_routes,
            &mut input_connections,
            false,
        );
        let mut c = conf_mutex.lock().unwrap();
        let system_conf = c.systems.get_mut(&metadata.system).expect("invalid system");
        let bytes = if system_conf.provide_timetable {
            query_on_timetable(system_conf, metadata)
        } else {
            query_on_given(
                system_conf,
                &mut input_stations,
                &input_routes,
                &mut input_connections,
                metadata,
            )
        };
        println!("finished querying.");
        Response::from_data("application/octet-stream", bytes)
    });
}
