use stost::connection;
use stost::distribution_store;
use stost::wire::serde;
use stost::query::topocsa;

use std::collections::HashMap;


#[test]
fn it_compiles() {
    let mut store = distribution_store::Store::new();
    store.load_distributions("./data/de_db.csv");

    let bytes: Vec<u8> = serde::read_protobuf("./tests/fixtures/basic.pb");
    let mut stations: HashMap<String, connection::Station> = HashMap::new();
    let mut routes = HashMap::new();
    let mut connections = vec![];
    let (start_time, o, d, now) = serde::deserialize_protobuf(bytes, &mut stations, &mut routes, &mut connections);
    topocsa::query(&mut store, &mut connections, o, d, 0, 100, serde::to_mtime(now, start_time));
}
