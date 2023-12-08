use rouille::Response;
use std::io::Read;
use std::collections::HashMap;
use std::sync::Mutex;

use stost::distribution_store;
use stost::query;
use stost::connection;
use stost::wire::serde;


fn main() {
    let mut store = distribution_store::Store::new();
    store.load_distributions("./data/de_db.csv");
    let store_mutex = Mutex::new(store);

    println!("starting...");
    rouille::start_server("0.0.0.0:1234", move |request| {

        println!("receiving req...");
        let mut bytes: Vec<u8> = vec![];
        let result = request.data().unwrap().read_to_end(&mut bytes);
        if result.is_err() {
            panic!("{:?}", result);
        }
        let mut stations: HashMap<String, connection::Station> = HashMap::new();
        let mut routes = HashMap::new();
        let mut connections = vec![];
        let (start_time, o, d, now) = serde::deserialize_protobuf(bytes, &mut stations, &mut routes, &mut connections);
        let mut s = store_mutex.lock().unwrap();
        println!("querying...");       
        query::query(&mut s, &mut connections, o, d, 0, 100, serde::to_mtime(now, start_time));
        println!("finished querying.");
        let bytes = serde::serialize_protobuf(&connections, start_time);
        Response::from_data("application/octet-stream", bytes)
    });
}
