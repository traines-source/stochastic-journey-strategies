use std::{collections::HashMap, cell::RefCell};

use chrono;

use motis_nigiri::Timetable;

use crate::connection;

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

pub fn retrieve<'a, 'b>(t: &Timetable, stations: &'a mut Vec<connection::Station>, _routes: &'a mut Vec<connection::Route>, connections: &'b mut Vec<connection::Connection>) -> HashMap<(usize, u16), usize> {
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
        let mut conn = connection::Connection::new(
            id, c.route_idx.try_into().unwrap(), r.clasz.try_into().unwrap(), c.trip_id.try_into().unwrap(), false,
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
            contr.stop_to_group[station.0] = if station.1.parent_idx != 0 { station.1.parent_idx } else { station.0 };
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

fn geodist_meters(stop1: &connection::Station, stop2: &connection::Station) -> f32 {       
    let r = 6371e3;
    let x = (stop2.lon.to_radians()-stop1.lon.to_radians()) * ((stop1.lat.to_radians()+stop2.lat.to_radians())/2 as f32).cos();
    let y = stop2.lat.to_radians()-stop1.lat.to_radians();
    (x*x + y*y).sqrt() * r
}

const WALKING_METRES_PER_SECOND: f32 = 1.5;

pub fn shorten_footpaths(stations: &mut Vec<connection::Station>) {
    for i in 0..stations.len() {
        for j in 0..stations[i].footpaths.len() {
            let dur = (geodist_meters(&stations[i], &stations[stations[i].footpaths[j].target_location_idx])/WALKING_METRES_PER_SECOND/60.0).round() as u16;
            stations[i].footpaths[j].duration = std::cmp::min(std::cmp::max(dur, 1), stations[i].footpaths[j].duration);
        }
        stations[i].transfer_time = 1;
    }
}

pub fn load_realtime<F: FnMut(usize, bool, Option<usize>, Option<bool>, Option<i16>)>(gtfsrt_path: &str, t: &Timetable, transport_and_day_to_connection_id: &HashMap<(usize, u16), usize>, mut callback: F) {
    t.update_with_rt(gtfsrt_path, |e| callback(transport_and_day_to_connection_id[&(e.transport_idx, e.day_idx)]+e.stop_idx as usize, e.is_departure, e.location_idx, e.in_out_allowed, e.delay));
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