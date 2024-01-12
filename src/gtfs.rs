use std::collections::HashMap;

use chrono;

use motis_nigiri::Timetable;

use crate::connection;

use std::collections::HashSet;
use serde::{Serialize, Deserialize};
use rmps::Serializer;
use std::io::Write;
use std::fs;

use crate::distribution_store;
use crate::query::topocsa;

#[derive(Serialize, Deserialize, Debug)]
pub struct GtfsTimetable {
    pub stations: Vec<connection::Station>,
    pub connections: Vec<connection::Connection>,
    pub cut: HashSet<(usize, usize)>,
    pub labels: HashMap<usize, topocsa::ConnectionLabel>,
    pub transport_and_day_to_connection_id: HashMap<(usize, u16), usize>
}

pub fn load_timetable<'a, 'b>(gtfs_path: &str, start_date: chrono::NaiveDate, end_date: chrono::NaiveDate) -> Timetable {
    Timetable::load(gtfs_path, start_date, end_date)
}

pub fn retrieve<'a, 'b>(t: &Timetable, stations: &'a mut Vec<connection::Station>, _routes: &'a mut Vec<connection::Route>, connections: &'b mut Vec<connection::Connection>) -> HashMap<(usize, u16), usize> {
    let gtfs_locations = t.get_locations();
    for mut l in gtfs_locations {
        let mut station = connection::Station::new(l.id.to_string(), l.name.to_string(), vec![]);
        station.footpaths.append(&mut l.footpaths);
        stations.push(station);
    }
    let mut gtfs_connections = t.get_connections();
    for c in &mut gtfs_connections {
        let id = connections.len();
        assert_eq!(id, c.id);
        // TODO routes
        /*let route_idx = match stations_idx.get(&c.route_idx) {
            Some(idx) => idx,
            None => {
                let route_idx = routes.len();
                routes.push(connection::Route::new())
                t.get_route(c.route_idx);
                routes_idx.insert(c.route_idx, )
        }*/
        let r = t.get_route(c.route_idx);
        let from_idx = c.from_idx.try_into().unwrap();
        let to_idx = c.to_idx.try_into().unwrap();
        connections.push(connection::Connection::new(
            id, c.route_idx.try_into().unwrap(), r.clasz.try_into().unwrap(), c.trip_id.try_into().unwrap(), false,
            from_idx, c.departure.try_into().unwrap(), None,
            to_idx, c.arrival.try_into().unwrap(), None
        ));
        stations[from_idx].departures.borrow_mut().push(id);
    }
    gtfs_connections.into()
}

pub fn load_realtime<F: FnMut(usize, bool, i16, bool)>(gtfsrt_path: &str, t: &Timetable, transport_and_day_to_connection_id: &HashMap<(usize, u16), usize>, mut callback: F) {
    t.update_with_rt(gtfsrt_path, |e| callback(transport_and_day_to_connection_id[&(e.transport_idx, e.day_idx)]+e.stop_idx as usize, e.is_departure, e.delay, e.cancelled));
}

pub fn load_gtfs_cache(cache_path: &str) -> GtfsTimetable {
    let buf = std::fs::read(cache_path).unwrap();
    rmp_serde::from_slice(&buf).unwrap()
}