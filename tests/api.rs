#[macro_use]
extern crate assert_float_eq;

use std::collections::HashMap;

use stost::connection;
use stost::distribution_store;
use stost::wire::serde;
use stost::query::topocsa;
use stost::query::recursive;


#[test]
fn it_compiles() {
    let mut store = distribution_store::Store::new();
    store.load_distributions("./data/de_db.csv");

    let bytes: Vec<u8> = serde::read_protobuf("./tests/fixtures/basic.pb");
    let mut stations: HashMap<String, connection::Station> = HashMap::new();
    let mut routes = HashMap::new();
    let mut connections = vec![];
    let (start_time, o, d, now) = serde::deserialize_protobuf(bytes, &mut stations, &mut routes, &mut connections);
    let mut connections_clone = connections.clone();
    topocsa::query(&mut store, &mut connections, o, d, 0, 100, serde::to_mtime(now, start_time));
    recursive::query(&mut store, &mut connections_clone, o, d, 0, 100, serde::to_mtime(now, start_time));
    for c in connections {
        let a = connections_clone.get(c.id).unwrap().destination_arrival.borrow();
        let b = c.destination_arrival.borrow();
        if a.is_some() && a.as_ref().unwrap().exists() && b.is_some() && b.as_ref().unwrap().exists() {
            assert_float_absolute_eq!(a.as_ref().unwrap().mean, b.as_ref().unwrap().mean, 1e2);
        }
    }
}
