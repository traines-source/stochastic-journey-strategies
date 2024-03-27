use std::{collections::HashMap, cell::RefCell};

use chrono;

use motis_nigiri::Timetable;

use crate::connection::{self, Route};

use rustc_hash::FxHashSet;
use serde::{Serialize, Deserialize};
use rand::Rng;

#[derive(Serialize, Deserialize, Debug)]
pub struct GtfsTimetable {
    pub stations: Vec<connection::Station>,
    pub connections: Vec<connection::Connection>,
    pub cut: FxHashSet<(usize, usize)>,
    pub order: Vec<usize>,
    pub transport_and_day_to_connection_id: HashMap<(usize, u16), usize>
}

impl GtfsTimetable {
    pub fn new() -> GtfsTimetable {
        GtfsTimetable {
            stations: vec![],
            connections: vec![],
            cut: FxHashSet::default(),
            order: vec![],
            transport_and_day_to_connection_id: HashMap::new()
        }
    }
}

#[derive(Debug)]
pub struct StationContraction {
    pub stop_to_group: Vec<usize>,
    pub stop_to_group_idx: Vec<usize>,
    max_group_size: usize,
    pub transfer_times: Vec<u16>,
}

impl StationContraction {
    #[inline(always)]
    pub fn get_transfer_time(&self, from_stop_idx: usize, to_stop_idx: usize) -> u16 {
        self.transfer_times[from_stop_idx*self.max_group_size+self.stop_to_group_idx[to_stop_idx]]
    }
}

pub fn load_timetable<'a, 'b>(gtfs_path: &str, start_date: chrono::NaiveDate, end_date: chrono::NaiveDate) -> Timetable {
    Timetable::load(gtfs_path, start_date, end_date)
}

pub fn retrieve<'a, 'b>(t: &Timetable, stations: &'a mut Vec<connection::Station>, routes: &'a mut Vec<connection::Route>, connections: &'b mut Vec<connection::Connection>) -> HashMap<(usize, u16), usize> {
    let gtfs_locations = t.get_locations();
    for mut l in gtfs_locations {
        let mut station = connection::Station {
            id: l.id.to_string(), 
            name: l.name.to_string(),
            arrivals: vec![],
            departures: vec![],
            lat: l.lat,
            lon: l.lon,
            transfer_time: l.transfer_time,
            parent_idx: l.parent_idx,
            footpaths: vec![]
        };
        station.footpaths.append(&mut l.footpaths);
        stations.push(station);
    }
    let gtfs_routes = t.get_routes();
    for r in gtfs_routes {
        routes.push(Route::new(r.route_idx.to_string(), "".to_string(), r.clasz as i16));
    }
    let mut gtfs_connections = t.get_connections();
    for c in &mut gtfs_connections {
        let id = connections.len();
        assert_eq!(id, c.id);
        let route = routes.get_mut(c.route_idx).unwrap();
        if route.name == "" {
            route.name = c.name;
        }
        let from_idx = c.from_idx.try_into().unwrap();
        let to_idx = c.to_idx.try_into().unwrap();
        let mut conn = connection::Connection::new(
            id, c.route_idx.try_into().unwrap(), route.product_type, c.trip_id.try_into().unwrap(), false,
            from_idx, c.departure.try_into().unwrap(), None,
            to_idx, c.arrival.try_into().unwrap(), None
        );
        conn.departure.in_out_allowed = c.in_allowed;
        conn.arrival.in_out_allowed = c.out_allowed;
        connections.push(conn);
        stations[from_idx].departures.push(id);
        stations[to_idx].arrivals.push(id);
    }
    for station in stations {
        station.departures.sort_unstable_by(|a,b| connections[*a].departure.projected().cmp(&connections[*b].departure.projected()));
    }
    gtfs_connections.into()
}

pub fn sort_station_departures_asc(stations: &mut Vec<connection::Station>, connections: &[connection::Connection], order: &[usize]) {
    for station in stations {
        station.departures.sort_unstable_by(|a,b| connections[order[*a]].departure.projected().cmp(&connections[order[*b]].departure.projected()));
    }
}

pub fn get_station_contraction(stations: &[connection::Station]) -> StationContraction {
    let max_group_size = stations.iter().map(|s| s.footpaths.len()).max().unwrap()+1;
    let mut contr = StationContraction {
        stop_to_group: vec![0; stations.len()],
        stop_to_group_idx: vec![0; stations.len()],
        max_group_size: max_group_size,
        transfer_times: vec![0; stations.len()*max_group_size]
    };
    let mut max_dur = 0;
    let mut max_dur_info = "".to_owned();
    for station in stations.iter().enumerate() {
        if contr.stop_to_group[station.0] == 0 {
            contr.stop_to_group[station.0] = if station.1.parent_idx != 0 {
                station.1.parent_idx
            } else {
                station.0
            };
            contr.stop_to_group_idx[station.0] = 0;
            for f in station.1.footpaths.iter().enumerate() {
                contr.stop_to_group[f.1.target_location_idx] = contr.stop_to_group[station.0];
                contr.stop_to_group_idx[f.1.target_location_idx] = f.0+1;
                if max_dur < f.1.duration {
                    max_dur = f.1.duration;
                    max_dur_info = format!("{} {}", station.1.name, stations[f.1.target_location_idx].name);
                }
            }
        }
        contr.transfer_times[station.0*contr.max_group_size+contr.stop_to_group_idx[station.0]] = station.1.transfer_time;
        for f in station.1.footpaths.iter().enumerate() {
            contr.transfer_times[station.0*contr.max_group_size+contr.stop_to_group_idx[f.1.target_location_idx]] = f.1.duration;
        }
    }
    println!("max group size {} max dur {} between {}", contr.max_group_size, max_dur, max_dur_info);
    contr
}

fn to_connecion_id(e: &motis_nigiri::EventChange, transport_and_day_to_connection_id: &HashMap<(usize, u16), usize>) -> usize {
    let dep_offset = if e.is_departure { 0 } else { 1 };
    let initial_connection_of_transport = transport_and_day_to_connection_id[&(e.transport_idx, e.day_idx)];
    initial_connection_of_transport-dep_offset+e.stop_idx as usize
}

pub fn load_realtime<F: FnMut(usize, bool, Option<usize>, Option<bool>, Option<i16>)>(gtfsrt_path: &str, t: &Timetable, transport_and_day_to_connection_id: &HashMap<(usize, u16), usize>, mut callback: F) {
    t.update_with_rt(gtfsrt_path, |e| callback(to_connecion_id(&e, transport_and_day_to_connection_id), e.is_departure, e.location_idx, e.in_out_allowed, e.delay));
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
        cut: FxHashSet::default(),
        order: vec![],
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