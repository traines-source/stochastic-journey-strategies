pub mod recursive;
pub mod topocsa;

use crate::distribution_store;
use crate::connection;
use crate::types;

pub fn query<'a>(store: &'a mut distribution_store::Store, connections: &mut Vec<connection::Connection<'a>>, origin: &'a connection::Station, destination: &'a connection::Station, start_time: types::Mtime, max_time: types::Mtime, now: types::Mtime) {
    topocsa::prepare_and_query(store, connections, origin, destination, start_time, max_time, now, 0.0);
}