use crate::{
    connection::{Connection, Station, StopInfo}, distribution_store, gtfs::{sort_station_departures_asc, GtfsTimetable, StationContraction}, query::{topocsa, ConnectionLabel, Queriable, Query}, types
};
use motis_nigiri::Footpath;
use rstar::RTree;
use rustc_hash::FxHashSet;
use std::{cell::RefCell, collections::HashMap};

const WALKING_METRES_PER_SECOND: f64 = 1.5;
const MAX_WALKING_METRES: f64 = 5000.0;
pub const WALKING_MSG: &str = "walking";
pub const WALKING_PRODUCT_TYPE: i16 = 100;
pub const WALKING_RELEVANCE_THRESH: f32 = 0.01;
pub const WALKING_INITIAL_BUFFER_MINUTES: i32 = 3;

pub fn geodist_meters_string(stop1: &Station, stop2: &Station) -> String {
    format!("{}m", geodist_meters(stop1, stop2).round())
}

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
            if i == j {
                continue;
            }
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

pub fn create_materialized_initial_footpaths(origin_idx: usize, stations: &mut Vec<Station>, connections: &mut Vec<Connection>) {
    let mut walking_connections = vec![];

    for i in 0..stations[origin_idx].footpaths.len() {
        let target_idx = stations[origin_idx].footpaths[i].target_location_idx;
        let duration = stations[origin_idx].footpaths[i].duration;
        for j in 0..stations[target_idx].departures.len() {
            let id = connections.len() + walking_connections.len();
            let cid = stations[target_idx].departures[j];
            let c = &connections[cid];
            let arrival = StopInfo::new(c.departure.projected()-WALKING_INITIAL_BUFFER_MINUTES, None);
            let mut departure = arrival.clone();
            departure.scheduled -= duration.max(1) as i32;
            if departure.projected() < 0 {
                continue;
            }
            departure.in_out_allowed = false;
            stations.get_mut(origin_idx).unwrap().departures.push(id);
            stations.get_mut(target_idx).unwrap().arrivals.push(id);
            walking_connections.push(Connection {
                id: id,
                route_idx: id,
                trip_id: cid as i32,
                product_type: WALKING_PRODUCT_TYPE,
                from_idx: origin_idx,
                to_idx: target_idx,
                departure: departure,
                arrival: arrival,
                message: WALKING_MSG.to_string(),
                destination_arrival: RefCell::new(None),
            });
        }
    }
    connections.append(&mut walking_connections);
    for station in &mut *stations {
        station.departures.sort_unstable_by(|a,b| connections[*a].departure.projected().cmp(&connections[*b].departure.projected()));
    }
}

pub fn update_footpath_relevance(origin_idx: usize, destination_idx: usize, order: &[usize], connections: &[Connection]) {
    for c in connections.iter() {
        if c.product_type == WALKING_PRODUCT_TYPE {
            c.destination_arrival.borrow().as_ref().inspect(|da| da.relevance.set(
                if c.from_idx == origin_idx && c.to_idx == destination_idx {
                    1.0
                } else {
                    da.relevance.get()*connections[order[c.trip_id as usize]].destination_arrival.borrow().as_ref().unwrap().relevance.get()
                }
            ));
        }
    }
}

pub fn create_materialized_quadratic_footpaths(stations: &mut Vec<Station>, connections: &mut Vec<Connection>) {
    let mut walking_connections = vec![];
    for s in 0..stations.len() {
        for c in connections.iter() {
            if stations[c.to_idx].id == stations[s].id {
                continue;
            }
            let dist = geodist_meters(&stations[c.to_idx], &stations[s]);
            if dist < MAX_WALKING_METRES {
                walking_connections.push(create_walking_connection(
                    c,
                    connections.len() + walking_connections.len(),
                    stations,
                    dist,
                    s,
                ));
            }
        }
    }
    connections.append(&mut walking_connections);
}

pub struct StationLocation {
    station_idx: usize,
    lon: f64,
    lat: f64,
}

impl StationLocation {
    pub fn new(station_idx: usize, lon: f64, lat: f64) -> StationLocation {
        StationLocation {
            station_idx,
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
                .nearest_neighbor_iter_with_distance_2(&[stations[*s.0].lon, stations[*s.0].lat])
                .take_while(|(_p, dist)| *dist < MAX_WALKING_METRES.powi(2))
                .take(10)
                .map(|(p, _dist)| (p.station_idx, *s.1)),
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
    destination_idx: usize,
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
                new_stations.push(stations[orig_idx].clone_metadata());
                new_idx
            })
    };
    get_or_insert_new_station_idx(origin_idx, &mut new_stations);
    get_or_insert_new_station_idx(destination_idx, &mut new_stations);
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
        if weights_by_station_idx.is_empty() {
            new.destination_arrival = departure.destination_arrival.clone()
        }
        new_connections.push(new);
    }
    let mut walking_connections = vec![];
    /*for s1 in weights_by_station_idx.iter() {
        for s2 in weights_by_station_idx.iter() {
            if s1.0 == s2.0 {
                continue;
            }
            let from_idx = get_or_insert_new_station_idx(*s1.0, &mut new_stations);
            let to_idx = get_or_insert_new_station_idx(*s2.0, &mut new_stations);
            let dist = geodist_meters(&new_stations[from_idx], &new_stations[to_idx]);
            if dist < MAX_WALKING_METRES {
                new_stations
                    .get_mut(from_idx)
                    .unwrap()
                    .footpaths
                    .push(Footpath {
                        target_location_idx: to_idx,
                        duration: walking_duration(dist),
                    });
            }
        }
    }*/
    for s in weights_by_station_idx.iter() {
        for c in &new_connections {
            if new_stations[c.to_idx].id == stations[*s.0].id {
                continue;
            }
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
        new_stations_map[destination_id],
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

pub fn query_with_extended_walking(store: &mut distribution_store::Store, tt: &mut GtfsTimetable, query: Query, now: types::Mtime, contraction: &StationContraction, _rtree: &RTree<StationLocation>) -> (GtfsTimetable, usize, usize, Vec<Vec<ConnectionLabel>>) {
    let mut env = topocsa::Environment::new(
        store,
        &mut tt.connections,
        &tt.stations,
        &mut tt.cut,
        &mut tt.order,
        now,
        0.01,
        0.001,
        true,
        true,
    );
    env.set_station_contraction(contraction);
    println!("preprocessing...");
    env.preprocess();
    println!("start_time: {} now: {}", query.start_time, now);
    println!("querying...");
    let station_labels = env.query(query);
    let weights_by_station_idx =
        env.get_relevant_stations(query.origin_idx, query.destination_idx, &station_labels, false);
    if weights_by_station_idx.is_empty() {
        return (GtfsTimetable::new(), 0, 0, vec![])
    }
    println!("unextended: {}", weights_by_station_idx.len());
    /*relevant_stations_with_extended_walking(
        &mut weights_by_station_idx,
        &tt.stations,
        rtree,
    );*/
    println!("extended: {}", weights_by_station_idx.len());
    let connection_pairs = env.relevant_connection_pairs(query, &weights_by_station_idx, 10000);
    println!("creating relevant tt...");
    let walking_timetable = create_relevant_timetable_with_extended_walking(
        &mut tt.connections,
        &tt.stations,
        &tt.order,
        connection_pairs,
        &weights_by_station_idx,
        query.origin_idx,
        query.destination_idx
    );
    let mut walking_tt = walking_timetable.0;
    println!("conns incl. walking: {} relstops: {} greatest footpath set: {}", walking_tt.connections.len(), walking_tt.stations.len(), walking_tt.stations.iter().map(|s|s.footpaths.len()).max().unwrap());
    let mut rel_env = topocsa::Environment::new(
        store,
        &mut walking_tt.connections,
        &walking_tt.stations,
        &mut walking_tt.cut,
        &mut walking_tt.order,
        now,
        0.01,
        0.001,
        false,
        false,
    );
    rel_env.preprocess();
    println!("querying walking tt...");
    let walking_query = Query {
        origin_idx: walking_timetable.1,
        destination_idx: walking_timetable.2,
        start_time: query.start_time,
        max_time: query.max_time
    };
    let walking_station_labels = rel_env.query(walking_query);
    println!("{:?}", station_labels[contraction.stop_to_group[query.origin_idx]].last());
    println!("{:?}", walking_station_labels[walking_timetable.1].last());
    
    (walking_tt, walking_timetable.1, walking_timetable.2, walking_station_labels)
}