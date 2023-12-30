use rouille::Response;
use std::io::Read;
use std::collections::HashMap;
use std::sync::Mutex;

use stost::distribution_store;
use stost::query;
use stost::connection;
use stost::gtfs;
use stost::wire::serde;


fn main() {
    let mut store = distribution_store::Store::new();
    store.load_distributions("./data/de_db.csv");
    let store_mutex = Mutex::new(store);

    let mut ch_stations: Vec<connection::Station> = vec![];
    let mut ch_routes = vec![];
    let mut ch_connections = vec![];    
    gtfs::load("", chrono::NaiveDate::from_ymd_opt(2018, 12, 9).unwrap(), chrono::NaiveDate::from_ymd_opt(2019, 12, 9).unwrap(), &mut ch_stations, &mut ch_routes, &mut ch_connections);

    println!("starting...");
    rouille::start_server("0.0.0.0:1234", move |request| {

        println!("receiving req...");

        //let bytes: Vec<u8> = serde::read_protobuf("./tests/fixtures/basic.pb");
        let mut bytes: Vec<u8> = vec![];
        let result = request.data().unwrap().read_to_end(&mut bytes);
        if result.is_err() {
            panic!("{:?}", result);
        }
        //serde::write_protobuf(&bytes, "./basic.pb");
        let mut stations: Vec<connection::Station> = vec![];
        let mut routes = vec![];
        let mut connections = vec![];
        let (start_time, o_idx, d_idx, now, system) = serde::deserialize_protobuf(bytes, &mut stations, &mut routes, &mut connections, false);
        /*if system == "ch_sbb" {
            stations = ch_stations;
            routes = ch_routes;
            connections = ch_connections;
        }*/
        let mut s = store_mutex.lock().unwrap();
        println!("querying...");
        let o = &stations[o_idx];
        let d = &stations[d_idx];
        query::query(&mut s, &mut connections, &stations, o, d, 0, 100, serde::to_mtime(now, start_time));
        println!("finished querying.");
        let bytes = serde::serialize_protobuf(&stations, &routes, &connections, o, d, start_time);
        Response::from_data("application/octet-stream", bytes)
    });
}
