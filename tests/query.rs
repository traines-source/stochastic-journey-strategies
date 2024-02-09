#[macro_use]
extern crate assert_float_eq;

use stost::connection;
use stost::distribution_store;
use stost::distribution;


fn setup<'a>() -> (distribution_store::Store, connection::Route, connection::Station, connection::Station, connection::Station) {
    let store = distribution_store::Store::new();
    let route = connection::Route::new("1".to_string(), "route1".to_string(), 1);

    let cs1 = vec![];
    let cs2 = vec![];
    let cs3 = vec![];
    let station0 = connection::Station::new("1".to_string(), "station0".to_string(), cs1);
    let station1 = connection::Station::new("2".to_string(), "station1".to_string(), cs2);
    let station2 = connection::Station::new("3".to_string(), "station2".to_string(), cs3);
    
    (store, route, station0, station1, station2)
}

#[test]
fn non_stochastic() {
    let (mut store, route, mut station0, mut station1, mut station2) = setup();

    let c0 = connection::Connection::new(0, 0, 1, 1, false,
        0, 10, None,
        1, 16, None);
    
    let c1 = connection::Connection::new(1, 0, 1, 2, false,
        1, 20, None,
        2, 30, None);

    let c2 = connection::Connection::new(2, 0, 1, 3, false,
        1, 30, None,
        2, 40, None);
    let mut connections = vec![c0, c1, c2];
    station0.add_departure(0);
    station1.add_departure(1);
    station1.add_departure(2);
    let stations = vec![station0, station1, station2];

    stost::query::query(&mut store, &mut connections, &stations, &stations[0], &stations[2], 0, 100, 5);

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
    let (mut store, route, mut station0, mut station1, station2) = setup();

    let c0 = connection::Connection::new(0,  0, 1, 1, false,
        0, 10, None,
        1, 20, None);
    
    let c1 = connection::Connection::new(1, 0, 1, 2, false,
        1, 20, None,
        2, 30, None);
    
    let mut connections = vec![c0, c1];

    station0.add_departure(0);
    station1.add_departure(1);
    let stations = vec![station0, station1, station2];

    stost::query::query(&mut store, &mut connections, &stations, &stations[0], &stations[2], 0, 100, 5);

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
    let (mut store, route, mut station0, mut station1, station2) = setup();

    let c0 = connection::Connection::new(0, 0, 1, 1, false,
        0, 10, None,
        1, 20, None);
    
    let c1 = connection::Connection::new(1, 0, 1, 1, false,
        1, 20, None,
        2, 30, None);
    
    let mut connections = vec![c0, c1];

    station0.add_departure(0);
    station1.add_departure(1);
    let stations = vec![station0, station1, station2];

    stost::query::query(&mut store, &mut connections, &stations, &stations[0], &stations[2], 0, 100, 5);
    
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
    let (mut store, route, mut station0, mut station1, mut station2) = setup();

    let c0 = connection::Connection::new(0, 0, 1, 1, false,
        0, 10, None,
        1, 16, None);
    
    let c1 = connection::Connection::new(1, 0, 1, 2, false,
        1, 20, Some(0),
        2, 30, None);

    let c2 = connection::Connection::new(2, 0, 1, 3, false,
        1, 30, None,
        2, 40, None);

    let mut connections = vec![c0, c1, c2];
    
    station0.add_departure(0);
    station1.add_departure(1);
    station1.add_departure(2);
    let stations = vec![station0, station1, station2];
    
    let mut d = distribution::Distribution::uniform(0, 1);
    d.feasible_probability = 0.5;
    store.insert_from_distribution(0..5, 0..20, true, 1, d);

    stost::query::query(&mut store, &mut connections, &stations, &stations[0], &stations[2], 0, 100, 5);

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
    let (mut store, route, mut station0, mut station1, station2) = setup();

    let c0 = connection::Connection::new(0, 0, 1, 1, false,
        0, 10, None,
        1, 15, Some(3));
    
    let c1 = connection::Connection::new(1, 0, 1, 2, false,
        1, 20, None,
        2, 30, None);
    
    let c2 = connection::Connection::new(2, 0, 1, 3, false,
        1, 30, None,
        2, 40, Some(1));
    
    let mut connections = vec![c0, c1, c2];

    station0.add_departure(0);
    station1.add_departure(1);
    station1.add_departure(2);
    let stations = vec![station0, station1, station2];
    
    store.insert_from_distribution(0..5, 0..15, false, 1, distribution::Distribution::uniform(-5, 10));
    store.insert_from_distribution(0..5, 35..45, false, 1, distribution::Distribution::uniform(-2, 6));

    stost::query::query(&mut store, &mut connections, &stations, &stations[0], &stations[2], 0, 100, 5);

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
    let (mut store, route, mut station0, mut station1, station2) = setup();

    let c0 = connection::Connection::new(0, 0, 1, 1, false,
        0, 10, Some(0),
        1, 12, Some(0));
    
    let c1 = connection::Connection::new(1, 0, 1, 2, false,
        1, 14, Some(0),
        0, 16, Some(0));

    let c2 = connection::Connection::new(2, 0, 1, 3, false,
        1, 20, None,
        2, 30, Some(0));
    
    let mut connections = vec![c0, c1, c2];

    station0.add_departure(0);
    station1.add_departure(1);
    station1.add_departure(2);
    let stations = vec![station0, station1, station2];
    
    store.insert_from_distribution(0..5, 0..20, false, 1, distribution::Distribution::uniform(-5, 9));
    store.insert_from_distribution(0..5, 0..20, true, 1, distribution::Distribution::uniform(-5, 9));

    stost::query::query(&mut store, &mut connections, &stations, &stations[0], &stations[2], 0, 100, 5);

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
    let (mut store, route, mut station0, mut station1, station2) = setup();

    let c0 = connection::Connection::new(0, 0, 1, 1, false,
        0, 10, Some(0),
        1, 12, Some(0));

    let c3 = connection::Connection::new(3, 0, 1, 4, false,
        0, 5, None,
        1, 7, None);
    
    let c1 = connection::Connection::new(1, 0, 1, 2, false,
        1, 8, Some(0),
        0, 10, Some(0));

    let c2 = connection::Connection::new(2, 0, 1, 3, false,
        1, 20, None,
        2, 30, Some(0));

    let c4 = connection::Connection::new(4, 0, 1, 4, false,
        0, 30, Some(4),
        2, 60, Some(3));
    
    let mut connections = vec![c0, c1, c2, c3, c4];
    
    station0.add_departure(0);
    station0.add_departure(3);
    station1.add_departure(1);
    station1.add_departure(2);
    station0.add_departure(4);
    let stations = vec![station0, station1, station2];
    
    store.insert_from_distribution(0..5, 0..20, false, 1, distribution::Distribution::uniform(-5, 8));
    store.insert_from_distribution(0..5, 0..20, true, 1, distribution::Distribution::uniform(-5, 9));

    stost::query::query(&mut store, &mut connections, &stations, &stations[0], &stations[2], 0, 100, 5);

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
    let (mut store, route, mut station0, mut station1, mut station2) = setup();

    let c0 = connection::Connection::new(0, 0, 1, 0, false,
        1, 8, None,
        0, 9, None);

    let c1 = connection::Connection::new(1, 0, 1, 1, false,
        0, 10, Some(0),
        1, 11, None);

    let c2 = connection::Connection::new(2, 0, 1, 2, false,
        1, 12, None,
        2, 13, None);

    let c3 = connection::Connection::new(3, 0, 1, 3, false,
        0, 11, Some(0),
        2, 14, None);

    let c4 = connection::Connection::new(4, 0, 1, 4, false,
        2, 15, None,
        0, 16, Some(0));

    let c5 = connection::Connection::new(5, 0, 1, 4, false,
        2, 16, None,
        0, 17, Some(0));
    
    let mut connections = vec![c0, c1, c2, c3, c4, c5];
    
    station1.add_departure(0);
    station0.add_departure(1);
    station1.add_departure(2);
    station0.add_departure(3);
    station2.add_departure(4);
    station2.add_departure(5);
    let stations = vec![station0, station1, station2];

    store.insert_from_distribution(0..5, 0..20, false, 1, distribution::Distribution::uniform(-5, 9));
    store.insert_from_distribution(0..5, 0..20, true, 1, distribution::Distribution::uniform(-5, 9));

    stost::query::query(&mut store, &mut connections, &stations, &stations[0], &stations[2], 0, 100, 5);

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
    let (mut store, route, mut station0, mut station1, station2) = setup();

    let c0 = connection::Connection::new(0, 0, 1, 0, false,
        0, 8, None,
        1, 9, None);

    let c1 = connection::Connection::new(1, 0, 1, 1, false,
        1, 9, Some(1),
        2, 13, None);

    let c2 = connection::Connection::new(2, 0, 1, 2, false,
        1, 11, None,
        0, 12, None);

    let c3 = connection::Connection::new(3, 0, 1, 3, false,
        0, 13, None,
        1, 13, Some(1));
    
    let mut connections = vec![c0, c1, c2, c3];
    
    station0.add_departure(0);
    station1.add_departure(1);
    station1.add_departure(2);
    station0.add_departure(3);
    let stations = vec![station0, station1, station2];

    store.insert_from_distribution(1..5, 0..20, false, 1, distribution::Distribution::uniform(-5, 9));
    store.insert_from_distribution(1..5, 0..20, true, 1, distribution::Distribution::uniform(-5, 9));
    
    stost::query::query(&mut store, &mut connections, &stations, &stations[0], &stations[2], 0, 100, 5);

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
    let (mut store, route, mut station0, mut station1, station2) = setup();

    let c0 = connection::Connection::new(0, 0, 1, 1, false,
        0, 10, Some(0),
        1, 19, Some(0));

    let c1 = connection::Connection::new(1, 0, 1, 3, false,
        1, 20, None,
        2, 30, Some(0));   

    let mut connections = vec![c0, c1];
    
    station0.add_departure(0);
    station1.add_departure(1);
    let stations = vec![station0, station1, station2];
    
    store.insert_from_distribution(0..5, 0..40, false, 1, distribution::Distribution::uniform(-5, 8));
    store.insert_from_distribution(0..5, 0..40, true, 1, distribution::Distribution::uniform(-5, 8));

    stost::query::query(&mut store, &mut connections, &stations, &stations[0], &stations[2], 0, 100, 5);

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
    let (mut store, route, mut station0, mut station1, station2) = setup();

    let c0 = connection::Connection::new(0, 0, 1, 1, false,
        0, 10, None,
        1, 15, Some(3));
    
    let c1 = connection::Connection::new(1, 0, 1, 2, true,
        1, 20, None,
        2, 30, None);
    
    let c2 = connection::Connection::new(2, 0, 1, 3, false,
        1, 30, None,
        2, 40, Some(1));

    let mut connections = vec![c0, c1, c2];
    
    station0.add_departure(0);
    station1.add_departure(1);
    station1.add_departure(2);
    let stations = vec![station0, station1, station2];
    
    store.insert_from_distribution(0..5, 0..15, false, 1, distribution::Distribution::uniform(-5, 10));
    store.insert_from_distribution(0..5, 35..45, false, 1, distribution::Distribution::uniform(-2, 6));

    stost::query::query(&mut store, &mut connections, &stations, &stations[0], &stations[2], 0, 100, 5);
    
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
