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
    let mut connections = vec![];

    stost::query::query(&mut store, &mut connections, &station1, &station3, 0, 100, 5);
}

fn setup<'a>() -> (distribution_store::Store, connection::Route, connection::Station, connection::Station, connection::Station) {
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

    let c0 = connection::Connection::new(0, &route, 1, false,
        &station1, 10, None,
        &station2, 16, None);
    
    let c1 = connection::Connection::new(1, &route, 2, false,
        &station2, 20, None,
        &station3, 30, None);

    let c2 = connection::Connection::new(2, &route, 3, false,
        &station2, 30, None,
        &station3, 40, None);
    let mut connections = vec![c0, c1, c2];
    station1.add_departure(0);
    station2.add_departure(1);
    station2.add_departure(2);

    stost::query::query(&mut store, &mut connections, &station1, &station3, 0, 100, 5);

    let c0 = connections.iter().filter(|c| c.id == 0).last().unwrap();
    let c1 = connections.iter().filter(|c| c.id == 1).last().unwrap();    
    let c2 = connections.iter().filter(|c| c.id == 2).last().unwrap();

    let binding = c0.destination_arrival.borrow();
    let a = binding.as_ref().unwrap();
    assert_eq!(a.start, 30);
    assert_float_relative_eq!(a.mean, 30.0);
    assert_float_relative_eq!(a.feasible_probability, 1.0);
    //assert_eq!(a.histogram.len(), 11);
    assert_eq!(a.histogram.len(), 1);
    assert_float_relative_eq!(a.histogram[0], 1.0);
    //assert_float_relative_eq!(a.histogram[1], 0.0);
    //assert_float_relative_eq!(a.histogram[10], 0.0);

    let binding = c1.destination_arrival.borrow();
    let a = binding.as_ref().unwrap();
    assert_eq!(a.start, 30);
    assert_eq!(a.histogram.len(), 1);

    let binding = c2.destination_arrival.borrow();
    let a = binding.as_ref().unwrap();
    assert_eq!(a.start, 40);
    assert_eq!(a.histogram.len(), 1);
}

#[test]
fn zero_minutes_transfer() {
    let (mut store, route, station1, station2, station3) = setup();

    let c0 = connection::Connection::new(0, &route, 1, false,
        &station1, 10, None,
        &station2, 20, None);
    
    let c1 = connection::Connection::new(1, &route, 2, false,
        &station2, 20, None,
        &station3, 30, None);
    
    let mut connections = vec![c0, c1];

    station1.add_departure(0);
    station2.add_departure(1);

    stost::query::query(&mut store, &mut connections, &station1, &station3, 0, 100, 5);

    let c0 = connections.iter().filter(|c| c.id == 0).last().unwrap();
    let c1 = connections.iter().filter(|c| c.id == 1).last().unwrap();

    let binding = c0.destination_arrival.borrow();
    let a = binding.as_ref().unwrap();
    assert_eq!(a.exists(), false);

    let binding = c1.destination_arrival.borrow();
    let a = binding.as_ref().unwrap();
    assert_eq!(a.start, 30);
    assert_eq!(a.histogram.len(), 1);
}

#[test]
fn zero_minutes_transfer_same_trip() {
    let (mut store, route, station1, station2, station3) = setup();

    let c0 = connection::Connection::new(0, &route, 1, false,
        &station1, 10, None,
        &station2, 20, None);
    
    let c1 = connection::Connection::new(1, &route, 1, false,
        &station2, 20, None,
        &station3, 30, None);
    
    let mut connections = vec![c0, c1];

    station1.add_departure(0);
    station2.add_departure(1);

    stost::query::query(&mut store, &mut connections, &station1, &station3, 0, 100, 5);
    
    let c0 = connections.iter().filter(|c| c.id == 0).last().unwrap();
    let c1 = connections.iter().filter(|c| c.id == 1).last().unwrap();    
    
    let binding = c0.destination_arrival.borrow();
    let a = binding.as_ref().unwrap();
    assert_eq!(a.exists(), true);
    assert_eq!(a.start, 30);
    assert_float_relative_eq!(a.mean, 30.0);
    assert_float_relative_eq!(a.feasible_probability, 1.0);
    assert_eq!(a.histogram.len(), 1);
    assert_float_relative_eq!(a.histogram[0], 1.0);

    let binding = c1.destination_arrival.borrow();
    let a = binding.as_ref().unwrap();
    assert_eq!(a.start, 30);
    assert_eq!(a.histogram.len(), 1);
}

#[test]
fn with_cancelled_probability() {
    let (mut store, route, station1, station2, station3) = setup();

    let c0 = connection::Connection::new(0, &route, 1, false,
        &station1, 10, None,
        &station2, 16, None);
    
    let c1 = connection::Connection::new(1, &route, 2, false,
        &station2, 20, Some(0),
        &station3, 30, None);

    let c2 = connection::Connection::new(2, &route, 3, false,
        &station2, 30, None,
        &station3, 40, None);

    let mut connections = vec![c0, c1, c2];
    
    station1.add_departure(0);
    station2.add_departure(1);
    station2.add_departure(2);
    
    let mut d = distribution::Distribution::uniform(0, 1);
    d.feasible_probability = 0.5;
    store.insert_from_distribution(0..5, 0..20, true, 1, d);

    stost::query::query(&mut store, &mut connections, &station1, &station3, 0, 100, 5);

    let c0 = connections.iter().filter(|c| c.id == 0).last().unwrap();
    let c1 = connections.iter().filter(|c| c.id == 1).last().unwrap();    
    let c2 = connections.iter().filter(|c| c.id == 2).last().unwrap();

    let binding = c0.destination_arrival.borrow();
    let a = binding.as_ref().unwrap();
    assert_eq!(a.start, 30);
    assert_float_relative_eq!(a.mean, 35.0);
    assert_float_relative_eq!(a.feasible_probability, 1.0);
    assert_eq!(a.histogram.len(), 11);
    assert_float_relative_eq!(a.histogram[0], 0.5);
    assert_float_relative_eq!(a.histogram[1], 0.0);
    assert_float_relative_eq!(a.histogram[10], 0.5);

    let binding = c1.destination_arrival.borrow();
    let a = binding.as_ref().unwrap();
    assert_eq!(a.start, 30);
    assert_eq!(a.histogram.len(), 1);

    let binding = c2.destination_arrival.borrow();
    let a = binding.as_ref().unwrap();
    assert_eq!(a.start, 40);
    assert_eq!(a.histogram.len(), 1);
}

#[test]
fn with_uniform() {
    let (mut store, route, station1, station2, station3) = setup();

    let c0 = connection::Connection::new(0, &route, 1, false,
        &station1, 10, None,
        &station2, 15, Some(3));
    
    let c1 = connection::Connection::new(1, &route, 2, false,
        &station2, 20, None,
        &station3, 30, None);
    
    let c2 = connection::Connection::new(2, &route, 3, false,
        &station2, 30, None,
        &station3, 40, Some(1));
    
    let mut connections = vec![c0, c1, c2];

    station1.add_departure(0);
    station2.add_departure(1);
    station2.add_departure(2);
    
    store.insert_from_distribution(0..5, 0..15, false, 1, distribution::Distribution::uniform(-5, 10));
    store.insert_from_distribution(0..5, 35..45, false, 1, distribution::Distribution::uniform(-2, 6));

    stost::query::query(&mut store, &mut connections, &station1, &station3, 0, 100, 5);

    let c0 = connections.iter().filter(|c| c.id == 0).last().unwrap();

    let binding = c0.destination_arrival.borrow();
    let a = binding.as_ref().unwrap();
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

    let c0 = connection::Connection::new(0, &route, 1, false,
        &station1, 10, Some(0),
        &station2, 12, Some(0));
    
    let c1 = connection::Connection::new(1, &route, 2, false,
        &station2, 14, Some(0),
        &station1, 16, Some(0));

    let c2 = connection::Connection::new(2, &route, 3, false,
        &station2, 20, None,
        &station3, 30, Some(0));
    
    let mut connections = vec![c0, c1, c2];

    station1.add_departure(0);
    station2.add_departure(1);
    station2.add_departure(2);
    
    store.insert_from_distribution(0..5, 0..20, false, 1, distribution::Distribution::uniform(-5, 9));
    store.insert_from_distribution(0..5, 0..20, true, 1, distribution::Distribution::uniform(-5, 9));

    stost::query::query(&mut store, &mut connections, &station1, &station3, 0, 100, 5);

    let c0 = connections.iter().filter(|c| c.id == 0).last().unwrap();
    let c1 = connections.iter().filter(|c| c.id == 1).last().unwrap();    
    let c2 = connections.iter().filter(|c| c.id == 2).last().unwrap();

    let binding = c0.destination_arrival.borrow();
    let a = binding.as_ref().unwrap();
    assert_eq!(a.start, 30);
    assert_float_relative_eq!(a.mean, 30.0);
    assert_float_relative_eq!(a.feasible_probability, 1.0);
    assert_eq!(a.histogram.len(), 1);
    assert_float_relative_eq!(a.histogram[0], 1.0);
    
    let binding = c1.destination_arrival.borrow();
    let a = binding.as_ref().unwrap();
    assert_eq!(a.exists(), false);

    let binding = c2.destination_arrival.borrow();
    let a = binding.as_ref().unwrap();
    assert_eq!(a.start, 30);
    assert_float_relative_eq!(a.mean, 30.0);
    assert_float_relative_eq!(a.feasible_probability, 1.0);
    assert_eq!(a.histogram.len(), 1);
}


#[test]
fn infinite_loop_cut_at_lowest_reachability() {
    let (mut store, route, station1, station2, station3) = setup();


    let c0 = connection::Connection::new(0, &route, 1, false,
        &station1, 10, Some(0),
        &station2, 12, Some(0));

    let c3 = connection::Connection::new(3, &route, 4, false,
        &station1, 5, None,
        &station2, 7, None);
    
    let c1 = connection::Connection::new(1, &route, 2, false,
        &station2, 8, Some(0),
        &station1, 10, Some(0));

    let c2 = connection::Connection::new(2, &route, 3, false,
        &station2, 20, None,
        &station3, 30, Some(0));

    let c4 = connection::Connection::new(4, &route, 4, false,
        &station1, 30, Some(4),
        &station3, 60, Some(3));
    
    let mut connections = vec![c0, c1, c2, c3, c4];
    
    station1.add_departure(0);
    station1.add_departure(3);
    station2.add_departure(1);
    station2.add_departure(2);
    station1.add_departure(4);
    
    store.insert_from_distribution(0..5, 0..20, false, 1, distribution::Distribution::uniform(-5, 8));
    store.insert_from_distribution(0..5, 0..20, true, 1, distribution::Distribution::uniform(-5, 9));

    stost::query::query(&mut store, &mut connections, &station1, &station3, 0, 100, 5);

    let c0 = connections.iter().filter(|c| c.id == 0).last().unwrap();
    let c1 = connections.iter().filter(|c| c.id == 1).last().unwrap();    
    let c2 = connections.iter().filter(|c| c.id == 2).last().unwrap();
    let c3 = connections.iter().filter(|c| c.id == 3).last().unwrap();
    let c4 = connections.iter().filter(|c| c.id == 4).last().unwrap();

    let binding = c3.destination_arrival.borrow();
    let a = binding.as_ref().unwrap();
    assert_eq!(a.start, 30);
    assert_float_relative_eq!(a.mean, 30.0);
    assert_float_relative_eq!(a.feasible_probability, 1.0);
    assert_eq!(a.histogram.len(), 1);
    assert_float_relative_eq!(a.histogram[0], 1.0);

    let binding = c0.destination_arrival.borrow();
    let a = binding.as_ref().unwrap();
    assert_eq!(a.start, 30);
    assert_float_relative_eq!(a.mean, 30.0);
    assert_float_relative_eq!(a.feasible_probability, 1.0);
    assert_eq!(a.histogram.len(), 1);
    assert_float_relative_eq!(a.histogram[0], 1.0);
    
    let binding = c1.destination_arrival.borrow();
    let a = binding.as_ref().unwrap();
    assert_eq!(a.start, 30);
    assert_float_relative_eq!(a.mean, 46.499992);
    assert_float_relative_eq!(a.feasible_probability, 1.0);
    assert_eq!(a.histogram.len(), 34);

    let binding = c2.destination_arrival.borrow();
    let a = binding.as_ref().unwrap();
    assert_eq!(a.start, 30);
    assert_float_relative_eq!(a.mean, 30.0);
    assert_float_relative_eq!(a.feasible_probability, 1.0);
    assert_eq!(a.histogram.len(), 1);

    let binding = c4.destination_arrival.borrow();
    let a = binding.as_ref().unwrap();
    assert_eq!(a.start, 63);
    assert_float_relative_eq!(a.mean, 63.0);
    assert_float_relative_eq!(a.feasible_probability, 1.0);
    assert_eq!(a.histogram.len(), 1);
}


#[test]
fn infinite_loop_cut_and_revisit() {
    let (mut store, route, station1, station2, station3) = setup();

    let c0 = connection::Connection::new(0, &route, 0, false,
        &station2, 8, None,
        &station1, 9, None);

    let c1 = connection::Connection::new(1, &route, 1, false,
        &station1, 10, Some(0),
        &station2, 11, None);

    let c2 = connection::Connection::new(2, &route, 2, false,
        &station2, 12, None,
        &station3, 13, None);

    let c3 = connection::Connection::new(3, &route, 3, false,
        &station1, 11, Some(0),
        &station3, 14, None);

    let c4 = connection::Connection::new(4, &route, 4, false,
        &station3, 15, None,
        &station1, 16, Some(0));

    let c5 = connection::Connection::new(5, &route, 4, false,
        &station3, 16, None,
        &station1, 17, Some(0));
    
    let mut connections = vec![c0, c1, c2, c3, c4, c5];
    
    station2.add_departure(0);
    station1.add_departure(1);
    station2.add_departure(2);
    station1.add_departure(3);
    station3.add_departure(4);
    station3.add_departure(5);

    store.insert_from_distribution(0..5, 0..20, false, 1, distribution::Distribution::uniform(-5, 9));
    store.insert_from_distribution(0..5, 0..20, true, 1, distribution::Distribution::uniform(-5, 9));

    stost::query::query(&mut store, &mut connections, &station1, &station3, 0, 100, 5);

    //let c0 = connections.iter().filter(|c| c.id == 0).last().unwrap();
    let c1 = connections.iter().filter(|c| c.id == 1).last().unwrap();    
    let c2 = connections.iter().filter(|c| c.id == 2).last().unwrap();
    let c3 = connections.iter().filter(|c| c.id == 3).last().unwrap();
    //let c4 = connections.iter().filter(|c| c.id == 4).last().unwrap();
    //let c5 = connections.iter().filter(|c| c.id == 5).last().unwrap();

    /*let binding = c0.destination_arrival.borrow();
    let a = binding.as_ref().unwrap();
    assert_eq!(a.start, 13);
    assert_eq!(a.histogram.len(), 2);*/

    let binding = c1.destination_arrival.borrow();
    let a = binding.as_ref().unwrap();
    assert_eq!(a.start, 13);
    assert_eq!(a.histogram.len(), 1);

    let binding = c2.destination_arrival.borrow();
    let a = binding.as_ref().unwrap();
    assert_eq!(a.start, 13);
    assert_eq!(a.histogram.len(), 1);

    let binding = c3.destination_arrival.borrow();
    let a = binding.as_ref().unwrap();
    assert_eq!(a.start, 14);
    assert_eq!(a.histogram.len(), 1);

    /*let binding = c4.destination_arrival.borrow();
    let a = binding.as_ref().unwrap();
    assert_eq!(a.exists(), false);
    assert_eq!(a.feasible_probability, 0.0);

    let binding = c5.destination_arrival.borrow();
    let a = binding.as_ref().unwrap();
    assert_eq!(a.exists(), false);
    assert_eq!(a.feasible_probability, 0.0);*/
}


#[test]
fn revisit_completed() {
    let (mut store, route, station1, station2, station3) = setup();

    let c0 = connection::Connection::new(0, &route, 0, false,
        &station1, 8, None,
        &station2, 9, None);

    let c1 = connection::Connection::new(1, &route, 1, false,
        &station2, 9, Some(1),
        &station3, 13, None);

    let c2 = connection::Connection::new(2, &route, 2, false,
        &station2, 11, None,
        &station1, 12, None);

    let c3 = connection::Connection::new(3, &route, 3, false,
        &station1, 13, None,
        &station2, 13, Some(1));
    
    let mut connections = vec![c0, c1, c2, c3];
    
    station1.add_departure(0);
    station2.add_departure(1);
    station2.add_departure(2);
    station1.add_departure(3);

    store.insert_from_distribution(1..5, 0..20, false, 1, distribution::Distribution::uniform(-5, 9));
    store.insert_from_distribution(1..5, 0..20, true, 1, distribution::Distribution::uniform(-5, 9));
    
    stost::query::query(&mut store, &mut connections, &station1, &station3, 0, 100, 5);

    let c0 = connections.iter().filter(|c| c.id == 0).last().unwrap();
    let c1 = connections.iter().filter(|c| c.id == 1).last().unwrap();    
    let c2 = connections.iter().filter(|c| c.id == 2).last().unwrap();
    let c3 = connections.iter().filter(|c| c.id == 3).last().unwrap();

    let binding = c0.destination_arrival.borrow();
    let a = binding.as_ref().unwrap();
    assert_eq!(a.start, 13);
    assert_eq!(a.histogram.len(), 1);
    assert!(a.feasible_probability > 0.2); // depends on ordering of c1 and c2

    let binding = c1.destination_arrival.borrow();
    let a = binding.as_ref().unwrap();
    assert_eq!(a.start, 13);
    assert_eq!(a.histogram.len(), 1);
    assert_eq!(a.feasible_probability, 1.0);

    let binding = c2.destination_arrival.borrow();
    let a = binding.as_ref().unwrap();
    assert_eq!(a.start, 13);
    assert_eq!(a.histogram.len(), 1);
    assert_eq!(a.feasible_probability, 0.123456776);

    let binding = c3.destination_arrival.borrow();
    let a = binding.as_ref().unwrap();
    assert_eq!(a.start, 13);
    assert_eq!(a.histogram.len(), 1);
    assert_eq!(a.feasible_probability, 0.123456776);
}


#[test]
fn partial_feasibility() {
    let (mut store, route, station1, station2, station3) = setup();


    let c0 = connection::Connection::new(0, &route, 1, false,
        &station1, 10, Some(0),
        &station2, 19, Some(0));

    let c1 = connection::Connection::new(1, &route, 3, false,
        &station2, 20, None,
        &station3, 30, Some(0));   

    let mut connections = vec![c0, c1];
    
    station1.add_departure(0);
    station2.add_departure(1);
    
    store.insert_from_distribution(0..5, 0..40, false, 1, distribution::Distribution::uniform(-5, 8));
    store.insert_from_distribution(0..5, 0..40, true, 1, distribution::Distribution::uniform(-5, 8));

    stost::query::query(&mut store, &mut connections, &station1, &station3, 0, 100, 5);

    let c0 = connections.iter().filter(|c| c.id == 0).last().unwrap();
    let c1 = connections.iter().filter(|c| c.id == 1).last().unwrap();    

    let binding = c0.destination_arrival.borrow();
    let a = binding.as_ref().unwrap();
    assert_eq!(a.start, 25);
    assert_float_relative_eq!(a.mean, 28.5);
    assert_float_relative_eq!(a.feasible_probability, 0.75);
    assert_eq!(a.histogram.len(), 8);
    assert_float_relative_eq!(a.histogram[0], 0.125);
    assert_float_absolute_eq!(a.histogram.iter().sum::<f32>(), 1.0, 1e-3);
    
 
    let binding = c1.destination_arrival.borrow();
    let a = binding.as_ref().unwrap();
    assert_eq!(a.start, 25);
    assert_float_relative_eq!(a.mean, 28.5);
    assert_float_relative_eq!(a.feasible_probability, 1.0);
    assert_eq!(a.histogram.len(), 8);
    assert_float_relative_eq!(a.histogram[0], 0.125);
    assert_float_absolute_eq!(a.histogram.iter().sum::<f32>(), 1.0, 1e-3);
}

#[test]
fn with_cancelled() {
    let (mut store, route, station1, station2, station3) = setup();

    let c0 = connection::Connection::new(0, &route, 1, false,
        &station1, 10, None,
        &station2, 15, Some(3));
    
    let c1 = connection::Connection::new(1, &route, 2, true,
        &station2, 20, None,
        &station3, 30, None);
    
    let c2 = connection::Connection::new(2, &route, 3, false,
        &station2, 30, None,
        &station3, 40, Some(1));

    let mut connections = vec![c0, c1, c2];
    
    station1.add_departure(0);
    station2.add_departure(1);
    station2.add_departure(2);
    
    store.insert_from_distribution(0..5, 0..15, false, 1, distribution::Distribution::uniform(-5, 10));
    store.insert_from_distribution(0..5, 35..45, false, 1, distribution::Distribution::uniform(-2, 6));

    stost::query::query(&mut store, &mut connections, &station1, &station3, 0, 100, 5);
    
    let c0 = connections.iter().filter(|c| c.id == 0).last().unwrap();
    let c1 = connections.iter().filter(|c| c.id == 1).last().unwrap();
    
    let binding = c0.destination_arrival.borrow();
    let a = binding.as_ref().unwrap();
    assert_eq!(a.start, 39);
    assert_float_relative_eq!(a.mean, 41.5);
    assert_float_relative_eq!(a.feasible_probability, 1.0);
    assert_eq!(a.histogram.len(), 6);

    let binding = c1.destination_arrival.borrow();
    let a = binding.as_ref().unwrap();
    assert_eq!(a.exists(), false);
}
