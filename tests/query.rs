#[macro_use]
extern crate assert_float_eq;

use stost::connection;
use stost::distribution_store;
use stost::distribution;

#[test]
fn it_compiles() {
    let mut store = distribution_store::Store::new();
    let cs1 = vec![];
    let cs3 = vec![];
    let station1 = connection::Station::new("1".to_string(), "station1".to_string(), cs1);
    let station3 = connection::Station::new("3".to_string(), "station3".to_string(), cs3);
    
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

    let c1 = connection::Connection::new(&route, 1,
        &station1, 10, None,
        &station2, 16, None);
    
    let c2 = connection::Connection::new(&route, 2,
        &station2, 20, None,
        &station3, 30, None);

    let c3 = connection::Connection::new(&route, 3,
        &station2, 30, None,
        &station3, 40, None);
    
    station1.add_departure(&c1);
    station2.add_departure(&c2);
    station2.add_departure(&c3);

    stost::query::query(&mut store, &station1, &station3, 0, 100, 5);

    let a = c1.destination_arrival.borrow();
    assert_eq!(a.start, 30);
    assert_float_relative_eq!(a.mean, 30.0);
    assert_float_relative_eq!(a.feasible_probability, 1.0);
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

#[test]
fn zero_minutes_transfer() {
    let (mut store, route, station1, station2, station3) = setup();

    let c1 = connection::Connection::new(&route, 1,
        &station1, 10, None,
        &station2, 20, None);
    
    let c2 = connection::Connection::new(&route, 2,
        &station2, 20, None,
        &station3, 30, None);
    
    station1.add_departure(&c1);
    station2.add_departure(&c2);

    stost::query::query(&mut store, &station1, &station3, 0, 100, 5);

    let a = c1.destination_arrival.borrow();
    assert_eq!(a.exists(), false);

    let a = c2.destination_arrival.borrow();
    assert_eq!(a.start, 30);
    assert_eq!(a.histogram.len(), 1);
}

#[test]
fn zero_minutes_transfer_same_trip() {
    let (mut store, route, station1, station2, station3) = setup();

    let c1 = connection::Connection::new(&route, 1,
        &station1, 10, None,
        &station2, 20, None);
    
    let c2 = connection::Connection::new(&route, 1,
        &station2, 20, None,
        &station3, 30, None);
    
    station1.add_departure(&c1);
    station2.add_departure(&c2);

    stost::query::query(&mut store, &station1, &station3, 0, 100, 5);

    let a = c1.destination_arrival.borrow();
    assert_eq!(a.exists(), true);
    assert_eq!(a.start, 30);
    assert_float_relative_eq!(a.mean, 30.0);
    assert_float_relative_eq!(a.feasible_probability, 1.0);
    assert_eq!(a.histogram.len(), 1);
    assert_float_relative_eq!(a.histogram[0], 1.0);

    let a = c2.destination_arrival.borrow();
    assert_eq!(a.start, 30);
    assert_eq!(a.histogram.len(), 1);
}

#[test]
fn with_cancelled_probability() {
    let (mut store, route, station1, station2, station3) = setup();

    let c1 = connection::Connection::new(&route, 1,
        &station1, 10, None,
        &station2, 16, None);
    
    let c2 = connection::Connection::new(&route, 2,
        &station2, 20, Some(0),
        &station3, 30, None);

    let c3 = connection::Connection::new(&route, 3,
        &station2, 30, None,
        &station3, 40, None);
    
    station1.add_departure(&c1);
    station2.add_departure(&c2);
    station2.add_departure(&c3);

    let mut d = distribution::Distribution::uniform(0, 1);
    d.feasible_probability = 0.5;
    store.insert_from_distribution(0..5, 0..20, true, 1, d);

    stost::query::query(&mut store, &station1, &station3, 0, 100, 5);

    let a = c1.destination_arrival.borrow();
    assert_eq!(a.start, 30);
    assert_float_relative_eq!(a.mean, 35.0);
    assert_float_relative_eq!(a.feasible_probability, 1.0);
    assert_eq!(a.histogram.len(), 11);
    assert_float_relative_eq!(a.histogram[0], 0.5);
    assert_float_relative_eq!(a.histogram[1], 0.0);
    assert_float_relative_eq!(a.histogram[10], 0.5);

    let a = c2.destination_arrival.borrow();
    assert_eq!(a.start, 30);
    assert_eq!(a.histogram.len(), 1);

    let a = c3.destination_arrival.borrow();
    assert_eq!(a.start, 40);
    assert_eq!(a.histogram.len(), 1);
}

#[test]
fn with_uniform() {
    let (mut store, route, station1, station2, station3) = setup();

    let c1 = connection::Connection::new(&route, 1,
        &station1, 10, None,
        &station2, 15, Some(3));
    
    let c2 = connection::Connection::new(&route, 2,
        &station2, 20, None,
        &station3, 30, None);
    
    let c3 = connection::Connection::new(&route, 3,
        &station2, 30, None,
        &station3, 40, Some(1));
    
    station1.add_departure(&c1);
    station2.add_departure(&c2);
    station2.add_departure(&c3);
    
    store.insert_from_distribution(0..5, 0..15, false, 1, distribution::Distribution::uniform(-5, 10));
    store.insert_from_distribution(0..5, 35..45, false, 1, distribution::Distribution::uniform(-2, 6));

    stost::query::query(&mut store, &station1, &station3, 0, 100, 5);
    let a = c1.destination_arrival.borrow();
    assert_eq!(a.start, 30);
    assert_float_relative_eq!(a.mean, 33.45);
    assert_float_relative_eq!(a.feasible_probability, 1.0);
    assert_eq!(a.histogram.len(), 15);
    assert_float_relative_eq!(a.histogram[0], 0.7);
    assert_float_relative_eq!(a.histogram[1], 0.0);
    assert_float_relative_eq!(a.histogram[8], 0.0);
    assert_float_relative_eq!(a.histogram[9], 0.05);
    assert_float_relative_eq!(a.histogram[10], 0.05);
    assert_float_relative_eq!(a.histogram[11], 0.05);
    assert_float_relative_eq!(a.histogram[12], 0.05);
    assert_float_relative_eq!(a.histogram[13], 0.05);
    assert_float_relative_eq!(a.histogram[14], 0.05);
}

#[test]
fn infinite_loop() {
    let (mut store, route, station1, station2, station3) = setup();

    let c1 = connection::Connection::new(&route, 1,
        &station1, 10, Some(0),
        &station2, 12, Some(0));
    
    let c2 = connection::Connection::new(&route, 2,
        &station2, 14, Some(0),
        &station1, 16, Some(0));

    let c3 = connection::Connection::new(&route, 3,
        &station2, 20, None,
        &station3, 30, Some(0));
    
    station1.add_departure(&c1);
    station2.add_departure(&c2);
    station2.add_departure(&c3);
    
    store.insert_from_distribution(0..5, 0..20, false, 1, distribution::Distribution::uniform(-5, 9));
    store.insert_from_distribution(0..5, 0..20, true, 1, distribution::Distribution::uniform(-5, 9));

    stost::query::query(&mut store, &station1, &station3, 0, 100, 5);
    let a = c1.destination_arrival.borrow();
    assert_eq!(a.start, 30);
    assert_float_relative_eq!(a.mean, 30.0);
    assert_float_relative_eq!(a.feasible_probability, 1.0);
    assert_eq!(a.histogram.len(), 1);
    assert_float_relative_eq!(a.histogram[0], 1.0);
    
    let a = c2.destination_arrival.borrow();
    assert_eq!(a.exists(), false);

    let a = c3.destination_arrival.borrow();
    assert_eq!(a.start, 30);
    assert_float_relative_eq!(a.mean, 30.0);
    assert_float_relative_eq!(a.feasible_probability, 1.0);
    assert_eq!(a.histogram.len(), 1);
}
