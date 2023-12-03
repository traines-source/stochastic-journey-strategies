use crate::distribution_store;
use crate::connection;
use crate::types;

pub fn query<'a>(store: &'a mut distribution_store::Store, origin: &'a connection::Station<'a>, destination: &'a connection::Station<'a>, start_time: types::Mtime, max_time: types::Mtime, now: types::Mtime) {
    //recursive::query(store, origin, destination, start_time, max_time, now);
}