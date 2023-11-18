use stost::query;
use stost::connection;
use stost::distribution_store;

#[test]
fn it_compiles() {
    let mut store = distribution_store::Store::new();
    let route = connection::Route::new("1".to_string(), "route1".to_string(), 1);
    let mut cs1 = vec![];
    let mut cs2 = vec![];
    let mut cs3 = vec![];
    let mut station1 = connection::Station::new("1".to_string(), "station1".to_string(), &mut cs1);
    let mut station2 = connection::Station::new("2".to_string(), "station2".to_string(), &mut cs2);
    let mut station3 = connection::Station::new("3".to_string(), "station3".to_string(), &mut cs3);
    
    stost::query::query(&mut store, &mut station1, &mut station3, 0, 100, 5);
}

