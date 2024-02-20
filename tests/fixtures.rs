#[macro_use]
extern crate assert_float_eq;

use std::collections::HashMap;
use std::collections::HashSet;
use rustc_hash::FxHashSet;

use stost::connection;
use stost::distribution_store;
use stost::wire::serde;
use stost::query::topocsa;
use stost::query::recursive;

fn compare_connections(original: &[connection::Connection], new: &[connection::Connection]) {
    let mut i = 0;
    for c in new {
        i+= 1;
        
        let orig = original.get(c.id).unwrap();
        let a = orig.destination_arrival.borrow();
        let b = c.destination_arrival.borrow();
        println!("{} {} {} {} {} {:?}", i, orig.id, orig.route_idx, orig.from_idx, orig.departure.scheduled, orig.departure.delay);
        if a.is_some() && a.as_ref().unwrap().exists() || b.is_some() && b.as_ref().unwrap().exists() {
            assert_float_absolute_eq!(a.as_ref().unwrap().mean(), b.as_ref().unwrap().mean(), 1.0);
            assert_float_absolute_eq!(a.as_ref().unwrap().feasible_probability, b.as_ref().unwrap().feasible_probability, 0.01);
        }
    }
}

#[test]
#[ignore]
fn topocsa_recursive_identical() {
    let mut store = distribution_store::Store::new();
    store.load_distributions("./tests/fixtures/de_db.csv");

    let bytes: Vec<u8> = serde::read_protobuf("./tests/fixtures/basic.pb");
    let mut stations = vec![];
    let mut routes = vec![];
    let mut connections = vec![];
    let (start_time, o, d, now, _) = serde::deserialize_protobuf(bytes, &mut stations, &mut routes, &mut connections, false);
    let mut connections_clone = connections.clone();
    let mut cut = FxHashSet::default();
    topocsa::prepare_and_query(&mut store, &mut connections, &stations, &mut cut, o, d, 0, 100, serde::to_mtime(now, start_time), 0.0, false);
    recursive::query(&mut store, &mut connections_clone, &stations, &stations[o], &stations[d], 0, 100, serde::to_mtime(now, start_time), HashSet::from_iter(cut.into_iter()));

    compare_connections(&connections_clone, &connections);
}


#[test]
#[ignore]
fn topocsa_runs() {
    let mut store = distribution_store::Store::new();
    store.load_distributions("./tests/fixtures/de_db.csv");

    let bytes: Vec<u8> = serde::read_protobuf("./tests/fixtures/basic.pb");
    let mut stations = vec![];
    let mut routes = vec![];
    let mut connections = vec![];
    let (start_time, o, d, now, _) = serde::deserialize_protobuf(bytes, &mut stations, &mut routes, &mut connections, false);
    let mut cut = FxHashSet::default();
    topocsa::prepare_and_query(&mut store, &mut connections, &stations, &mut cut, o, d, 0, 100, serde::to_mtime(now, start_time), 0.0, false);

    let bytes: Vec<u8> = serde::read_protobuf("./tests/fixtures/basic_out.pb");
    let mut _stations = vec![];
    let mut _routes = vec![];
    let mut original_connections = vec![];
    let _ = serde::deserialize_protobuf(bytes, &mut _stations, &mut _routes, &mut original_connections, true);

    compare_connections(&original_connections, &connections);
}

#[test]
#[ignore]
fn recursive_runs() {
    let mut store = distribution_store::Store::new();
    store.load_distributions("./tests/fixtures/de_db.csv");

    let bytes: Vec<u8> = serde::read_protobuf("./tests/fixtures/basic.pb");
    let mut stations = vec![];
    let mut routes = vec![];
    let mut connections = vec![];
    let (start_time, o, d, now, _) = serde::deserialize_protobuf(bytes, &mut stations, &mut routes, &mut connections, false); 
    recursive::query(&mut store, &mut connections, &stations, &stations[o], &stations[d], 0, 100, serde::to_mtime(now, start_time), HashSet::new());
    let bytes = serde::serialize_protobuf(&stations, &routes, &connections, &stations[o], &stations[d], start_time);
    serde::write_protobuf(&bytes, "./tests/fixtures/basic_out.pb");
}