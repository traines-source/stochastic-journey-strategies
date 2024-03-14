pub mod recursive;
pub mod topocsa;
pub mod csameat;

use std::collections::HashMap;

use crate::gtfs::StationContraction;
use crate::distribution;
use crate::distribution_store;
use crate::connection;
use crate::types;

use rustc_hash::FxHashSet;

pub fn query<'a>(store: &'a mut distribution_store::Store, connections: &mut Vec<connection::Connection>, stations: &[connection::Station], origin: usize, destination: usize, start_time: types::Mtime, max_time: types::Mtime, now: types::Mtime) {
    let mut cut = FxHashSet::default();    
    topocsa::prepare_and_query(store, connections, stations, &mut cut, origin, destination, start_time, max_time, now, 0.0, false);
}


#[derive(Debug, Clone)]
pub struct ConnectionLabel {
    pub connection_idx: usize,
    pub destination_arrival: distribution::Distribution,
    pub prob_after: types::MFloat,
    pub departure_mean: types::MFloat
}

pub trait Query<'a> {
    fn set_station_contraction(&mut self, contr: &'a StationContraction);
    fn preprocess(&mut self);
    fn query(&mut self, _origin: usize, destination: usize, start_time: types::Mtime, max_time: types::Mtime) -> Vec<Vec<ConnectionLabel>>;
    fn relevant_stations(&mut self, origin_idx: usize, destination_idx: usize, station_labels: &[Vec<ConnectionLabel>]) -> HashMap<usize, types::MFloat>;
    fn relevant_connection_pairs(&mut self, weights_by_station_idx: &HashMap<usize, types::MFloat>) -> HashMap<i32, i32>;
    fn update(&mut self, connection_id: usize, is_departure: bool, location_idx: Option<usize>, in_out_allowed: Option<bool>, delay: Option<i16>);
}