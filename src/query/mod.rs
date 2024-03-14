pub mod recursive;
pub mod topocsa;
pub mod csameat;

use crate::distribution_store;
use crate::connection;
use crate::types;

use rustc_hash::FxHashSet;

pub fn query<'a>(store: &'a mut distribution_store::Store, connections: &mut Vec<connection::Connection>, stations: &[connection::Station], origin: usize, destination: usize, start_time: types::Mtime, max_time: types::Mtime, now: types::Mtime) {
    let mut cut = FxHashSet::default();    
    topocsa::prepare_and_query(store, connections, stations, &mut cut, origin, destination, start_time, max_time, now, 0.0, false);
}