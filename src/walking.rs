use crate::{
    connection::{Connection, Station},
    gtfs::{sort_station_departures_asc, GtfsTimetable},
    types,
};
use motis_nigiri::Footpath;
use rstar::RTree;
use rustc_hash::FxHashSet;
use std::{cell::RefCell, collections::HashMap};

const WALKING_METRES_PER_SECOND: f64 = 1.5;
const MAX_WALKING_METRES: f64 = 5000.0;
pub const WALKING_MSG: &str = "walking";

fn geodist_meters(stop1: &Station, stop2: &Station) -> f64 {
    lonlat_geodist_meters(stop1.lon, stop1.lat, stop2.lon, stop2.lat)
}

fn lonlat_geodist_meters(stop1_lon: f64, stop1_lat: f64, stop2_lon: f64, stop2_lat: f64) -> f64 {
    let r = 6371e3;
    let x = (stop2_lon.to_radians() - stop1_lon.to_radians())
        * ((stop1_lat.to_radians() + stop2_lat.to_radians()) / 2.0).cos();
    let y = stop2_lat.to_radians() - stop1_lat.to_radians();
    (x * x + y * y).sqrt() * r
}

fn walking_duration(dist: f64) -> u16 {
    (dist / WALKING_METRES_PER_SECOND / 60.0).round() as u16
}

pub fn shorten_footpaths(stations: &mut Vec<Station>) {
    for i in 0..stations.len() {
        for j in 0..stations[i].footpaths.len() {
            let dur = walking_duration(geodist_meters(
                &stations[i],
                &stations[stations[i].footpaths[j].target_location_idx],
            ));
            stations[i].footpaths[j].duration =
                std::cmp::min(std::cmp::max(dur, 1), stations[i].footpaths[j].duration);
        }
        stations[i].transfer_time = 1;
    }
}

pub fn create_quadratic_footpaths(stations: &mut Vec<Station>) {
    let mut ctr = 0;
    for i in 0..stations.len() {
        for j in 0..stations.len() {
            let dist = geodist_meters(&stations[i], &stations[j]);
            if dist < MAX_WALKING_METRES {
                stations[i].footpaths.push(Footpath {
                    target_location_idx: j,
                    duration: walking_duration(dist),
                });
                ctr += 1;
            }
        }
        stations[i].transfer_time = 1;
    }
    println!("Created {} footpaths", ctr);
}

pub struct StationLocation {
    station_id: usize,
    lon: f64,
    lat: f64,
}

impl StationLocation {
    pub fn new(station_id: usize, lon: f64, lat: f64) -> StationLocation {
        StationLocation {
            station_id,
            lon,
            lat,
        }
    }
}

impl rstar::RTreeObject for StationLocation {
    type Envelope = rstar::AABB<[f64; 2]>;

    fn envelope(&self) -> Self::Envelope {
        rstar::AABB::from_point([self.lon, self.lat])
    }
}

impl rstar::PointDistance for StationLocation {
    fn distance_2(
        &self,
        point: &<Self::Envelope as rstar::Envelope>::Point,
    ) -> <<Self::Envelope as rstar::Envelope>::Point as rstar::Point>::Scalar {
        lonlat_geodist_meters(self.lon, self.lat, point[0], point[1]).powi(2)
    }
}

pub fn init_rtree(stations: &[Station]) -> RTree<StationLocation> {
    RTree::bulk_load(
        stations
            .iter()
            .enumerate()
            .map(|s| StationLocation::new(s.0, s.1.lon, s.1.lat))
            .collect(),
    )
}

pub fn relevant_stations_with_extended_walking(
    weights_by_station_idx: &mut HashMap<usize, types::MFloat>,
    stations: &[Station],
    rtree: &RTree<StationLocation>,
) {
    let mut new_stops = vec![];
    for s in weights_by_station_idx.iter() {
        new_stops.extend(
            rtree
                .locate_within_distance(
                    [stations[*s.0].lon, stations[*s.0].lat],
                    MAX_WALKING_METRES.powi(2),
                )
                .map(|p| (p.station_id, *s.1)),
        );
    }
    weights_by_station_idx.extend(new_stops.into_iter());
}

pub fn create_relevant_timetable_with_extended_walking(
    connections: &[Connection],
    stations: &[Station],
    order: &[usize],
    connection_pairs: HashMap<i32, i32>,
    weights_by_station_idx: &HashMap<usize, types::MFloat>,
    origin_idx: usize,
    destination_idx: usize
) -> (GtfsTimetable, usize, usize) {
    let origin_id = &stations[origin_idx].id;
    let destination_id = &stations[destination_idx].id;
    let mut new_connections = vec![];
    let mut new_stations = vec![];
    let mut new_stations_map = HashMap::new();
    let mut get_or_insert_new_station_idx = |orig_idx: usize, new_stations: &mut Vec<Station>| {
        *new_stations_map
            .entry(&stations[orig_idx].id)
            .or_insert_with(|| {
                let new_idx = new_stations.len();
                new_stations[new_idx] = stations[orig_idx].clone_metadata();
                new_idx
            })
    };
    for pair in connection_pairs.iter() {
        let departure = &connections[order[*pair.1 as usize]];
        let arrival = &connections[order[*pair.0 as usize]];
        let mut new = departure.merge_pair(arrival, new_connections.len());
        let new_from_idx = get_or_insert_new_station_idx(departure.from_idx, &mut new_stations);
        let new_to_idx = get_or_insert_new_station_idx(arrival.to_idx, &mut new_stations);
        new_stations
            .get_mut(new_from_idx)
            .unwrap()
            .departures
            .push(new.id);
        new_stations
            .get_mut(new_to_idx)
            .unwrap()
            .arrivals
            .push(new.id);
        new.from_idx = new_from_idx;
        new.to_idx = new_to_idx;
        new_connections.push(new);
    }
    let mut walking_connections = vec![];
    for c in &new_connections {
        for s in weights_by_station_idx.iter() {
            let dist = geodist_meters(&new_stations[c.to_idx], &stations[*s.0]);
            if dist < MAX_WALKING_METRES {
                let to_idx = get_or_insert_new_station_idx(*s.0, &mut new_stations);
                walking_connections.push(create_walking_connection(
                    c,
                    new_connections.len() + walking_connections.len(),
                    &mut new_stations,
                    dist,
                    to_idx,
                ));
            }
        }
    }
    new_connections.append(&mut walking_connections);
    let new_order: Vec<usize> = (0..new_connections.len()).collect();
    sort_station_departures_asc(&mut new_stations, &new_connections, &new_order);
    (
        GtfsTimetable {
            stations: new_stations,
            connections: new_connections,
            cut: FxHashSet::default(),
            order: new_order,
            transport_and_day_to_connection_id: HashMap::new(),
        },
        new_stations_map[origin_id],
        new_stations_map[destination_id]
    )
}

fn create_walking_connection(
    c: &Connection,
    id: usize,
    new_stations: &mut Vec<Station>,
    dist: f64,
    to_idx: usize,
) -> Connection {
    let mut arrival = c.arrival.clone();
    arrival.scheduled += walking_duration(dist) as i32;
    new_stations.get_mut(c.to_idx).unwrap().departures.push(id);
    new_stations.get_mut(to_idx).unwrap().arrivals.push(id);
    Connection {
        id: id,
        route_idx: c.route_idx,
        trip_id: c.trip_id,
        product_type: c.product_type,
        from_idx: c.to_idx,
        to_idx: to_idx,
        departure: c.arrival.clone(),
        arrival: arrival,
        message: WALKING_MSG.to_string(),
        destination_arrival: RefCell::new(None),
    }
}
