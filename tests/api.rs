#[macro_use]
extern crate assert_float_eq;

use std::collections::HashMap;

use stost::connection;
use stost::distribution_store;
use stost::wire::serde;
use stost::query::topocsa;
use stost::query::recursive;


#[test]
#[ignore]
fn it_compiles() {
    let mut store = distribution_store::Store::new();
    store.load_distributions("./data/de_db.csv");

    let bytes: Vec<u8> = serde::read_protobuf("./tests/fixtures/basic.pb");
    let mut stations: HashMap<String, connection::Station> = HashMap::new();
    let mut routes = HashMap::new();
    let mut connections = vec![];
    let (start_time, o, d, now) = serde::deserialize_protobuf(bytes, &mut stations, &mut routes, &mut connections);
    let mut connections_clone = connections.clone();
    let cut = topocsa::query(&mut store, &mut connections, o, d, 0, 100, serde::to_mtime(now, start_time));
    recursive::query(&mut store, &mut connections_clone, o, d, 0, 100, serde::to_mtime(now, start_time), cut);

    let mut i = 0;
    for c in connections {
        i+= 1;
        //if i < 650 {continue;}
        
        let rc = connections_clone.get(c.id).unwrap();
        let a = rc.destination_arrival.borrow();
        let b = c.destination_arrival.borrow();
        //println!("{} {:?} {} {}", i, c, c.id, connections_clone.get(c.id).unwrap().id);
        if c.id == 32 {
            let re60 = connections_clone.get(954).unwrap();
            
            println!("sample {} {} {:?}", store.reachable_probability_conn(&re60, &c, 0), store.delay_distribution(&re60.arrival, false, 4, 0).mean, store.delay_distribution(&c.departure, true, 8, 0));
        }
        println!("{} {} {} {} {} {:?} {} {} {} {}", i, a.as_ref().unwrap().mean != b.as_ref().unwrap().mean, rc.route.name, rc.from.name, rc.departure.scheduled, rc.departure.delay, a.as_ref().unwrap().mean, b.as_ref().unwrap().mean, a.as_ref().unwrap().feasible_probability, b.as_ref().unwrap().feasible_probability);
            
        if a.is_some() && a.as_ref().unwrap().exists() || b.is_some() && b.as_ref().unwrap().exists() {
            //println!("{} {:?} {} {:?}", i, c, c.id, connections_clone.get(c.id).unwrap());
            assert_float_absolute_eq!(a.as_ref().unwrap().mean(), b.as_ref().unwrap().mean(), 1.0);
        } else {
            //println!("excluded {:?}, {:?}", a, b);
        }
    }
}
