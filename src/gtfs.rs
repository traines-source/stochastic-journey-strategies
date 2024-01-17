use std::collections::HashMap;

use chrono;

use motis_nigiri::Timetable;

use crate::connection;

use std::collections::HashSet;
use serde::{Serialize, Deserialize};
use rand::Rng;
use crate::query::topocsa;

#[derive(Serialize, Deserialize, Debug)]
pub struct GtfsTimetable {
    pub stations: Vec<connection::Station>,
    pub connections: Vec<connection::Connection>,
    pub cut: HashSet<(usize, usize)>,
    pub order: HashMap<usize, topocsa::ConnectionOrder>, // TODO rename
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

#[derive(Serialize, Deserialize, Debug)]
pub struct OriginDestinationSample {
    pub from_idx: usize,
    pub from_id: String,
    pub to_idx: usize,
    pub to_id: String
}

fn get_rand_conn_idx(connection_count: usize) -> usize {
    let mut rng = rand::thread_rng();
    rng.gen_range(0..connection_count)
}

pub fn create_simulation_samples(gtfs_path: &str, start_date: chrono::NaiveDate, end_date: chrono::NaiveDate) -> Vec<OriginDestinationSample> {
    let t = load_timetable(gtfs_path, start_date, end_date);
    let mut tt = GtfsTimetable {
        stations: vec![],
        connections: vec![],
        cut: HashSet::new(),
        order: HashMap::new(),
        transport_and_day_to_connection_id: HashMap::new()
    };
    let mut routes = vec![];
    retrieve(&t, &mut tt.stations, &mut routes, &mut tt.connections);
    let sample_count = 10000;
    let mut samples = vec![];
    for _i in 0..sample_count {
        let origin = tt.connections[get_rand_conn_idx(tt.connections.len())].from_idx;
        let destination = tt.connections[get_rand_conn_idx(tt.connections.len())].to_idx;
        samples.push(OriginDestinationSample {from_idx: origin, from_id: tt.stations[origin].id.clone(), to_idx: destination, to_id: tt.stations[destination].id.clone()});
    }
    samples
}