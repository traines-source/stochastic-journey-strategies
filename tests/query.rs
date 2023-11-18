#[macro_use]
extern crate assert_float_eq;

use stost::query;
use stost::connection;
use stost::distribution_store;

#[test]
fn it_compiles() {
    let mut store = distribution_store::Store::new();
    let route = connection::Route::new("1".to_string(), "route1".to_string(), 1);
    let mut cs1 = vec![];
    let mut cs3 = vec![];
    let mut station1 = connection::Station::new("1".to_string(), "station1".to_string(), cs1);
    let mut station3 = connection::Station::new("3".to_string(), "station3".to_string(), cs3);
    
    stost::query::query(&mut store, &station1, &station3, 0, 100, 5);
}

fn setup<'a>() -> (distribution_store::Store, connection::Route, connection::Station<'a>, connection::Station<'a>, connection::Station<'a>) {
    let store = distribution_store::Store::new();
    let route = connection::Route::new("1".to_string(), "route1".to_string(), 1);

    let cs1 = vec![];
    let cs2 = vec![];
    let cs3 = vec![];
    let station1 = connection::Station::new("1".to_string(), "station1".to_string(), cs1);
    let station2 = connection::Station::new("2".to_string(), "station2".to_string(), cs2);
    let station3 = connection::Station::new("3".to_string(), "station3".to_string(), cs3);
    
    (store, route, station1, station2, station3)
}


#[test]
fn non_stochastic() {
    let (mut store, route, station1, station2, station3) = setup();

    let c1 = connection::Connection::new(&route,
        &station1, 10, None,
        &station2, 16, None,
        0.0);
    
    let c2 = connection::Connection::new(&route,
        &station2, 20, None,
        &station3, 30, None,
        0.0);

    let c3 = connection::Connection::new(&route,
        &station2, 30, None,
        &station3, 40, None,
        0.0);
    
    station1.add_departure(&c1);
    station2.add_departure(&c2);
    station2.add_departure(&c3);

    stost::query::query(&mut store, &station1, &station3, 0, 100, 5);

    let a = c1.destination_arrival.borrow();
    assert_eq!(a.start, 30);
    assert_eq!(a.histogram.len(), 11);
    assert_float_relative_eq!(a.histogram[0], 1.0);
    assert_float_relative_eq!(a.histogram[1], 0.0);
    assert_float_relative_eq!(a.histogram[10], 0.0);

    let a = c2.destination_arrival.borrow();
    assert_eq!(a.start, 30);
    assert_eq!(a.histogram.len(), 1);

    let a = c3.destination_arrival.borrow();
    assert_eq!(a.start, 40);
    assert_eq!(a.histogram.len(), 1);
}

