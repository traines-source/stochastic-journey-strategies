#[macro_use]
extern crate assert_float_eq;


use stost::distribution_store;
use stost::query::topocsa;
use stost::gtfs;


#[test]
#[ignore]
fn gtfs() {
    let mut store = distribution_store::Store::new();
    store.load_distributions("./data/de_db.csv");

    let mut stations = vec![];
    let mut routes = vec![];
    let mut connections = vec![];
    gtfs::load("/gtfs/swiss-gtfs/2023-11-06", chrono::NaiveDate::from_ymd_opt(2023, 11, 1).unwrap(), chrono::NaiveDate::from_ymd_opt(2023, 11, 2).unwrap(), &mut stations, &mut routes, &mut connections);
    topocsa::prepare_and_query(&mut store, &mut connections, &stations, &stations[0], &stations[1], 0, 100, 0, 0.1);
}

