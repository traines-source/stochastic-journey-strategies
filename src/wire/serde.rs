use std::io::Write;
use std::fs;
use std::borrow::Cow;
use std::borrow::Borrow;
use std::collections::HashMap;
use indexmap::IndexMap;


use quick_protobuf::{MessageRead, MessageWrite, BytesReader, Writer};

use crate::gtfs::StationContraction;
use crate::types;
use crate::walking;
use crate::walking::WALKING_MSG;
use crate::walking::WALKING_PRODUCT_TYPE;
use crate::wire::wire;
use crate::connection;
use crate::distribution;

pub struct QueryMetadata {
    pub start_ts: i64,
    pub origin_id: String,
    pub origin_idx: usize,
    pub destination_id: String,
    pub destination_idx: usize,
    pub now: i64,
    pub system: String
}

pub fn write_protobuf(bytes: &Vec<u8>, filepath: &str) {
    let mut file = fs::OpenOptions::new()
        .create(true)
        .write(true)
        .open(filepath).expect("file not openable");
    file.write_all(bytes).expect("error writing file");
}

pub fn read_protobuf(filepath: &str) -> Vec<u8> {
    std::fs::read(filepath).unwrap()
}

pub fn to_mtime(t: i64, reference: i64) -> types::Mtime {
    ((t-reference) as f32/60.0).round() as types::Mtime
}

pub fn from_mtime(mtime: types::Mtime, reference: i64) -> i64 {
    (mtime*60) as i64 + reference
}

pub fn deserialize_protobuf<'a, 'b>(bytes: Vec<u8>, stations: &'a mut Vec<connection::Station>, routes: &'b mut Vec<connection::Route>, connections: &'b mut Vec<connection::Connection>, load_distributions: bool) -> QueryMetadata {
    let mut reader = BytesReader::from_bytes(&bytes);
    let request_message = wire::Message::from_reader(&mut reader, &bytes).expect("Cannot read Timetable");
        
    let timetable = request_message.timetable.as_ref().unwrap();
    let mut stations_idx: HashMap<&str, usize> = HashMap::new(); 
    for s in &timetable.stations {
        stations_idx.insert(s.id.borrow(), stations.len());
        stations.push(connection::Station {
			id: s.id.to_string(),
			name: s.name.to_string(),
			arrivals: vec![],
			departures: vec![],
			lat: s.lat as f64,
			lon: s.lon as f64,
			transfer_time: 1,
			parent_idx: 0,
			footpaths: vec![]
		});
    }
    let mut route_idx = 0;
    for r in &timetable.routes {
        routes.push(connection::Route {
            id: r.id.to_string(),
            direction: "".to_string(),
            name: "".to_string(),
            product_type: r.product_type as i16,
            message: "".to_string()
        });
        let mut trip_id = 0;
        for t in &r.trips {
            for c in &t.connections {
                let from_idx = stations_idx[&c.from_id as &str];
                let to_idx = stations_idx[&c.to_id as &str];
                let id = connections.len();
                let mut nc = connection::Connection::new(
                    id, route_idx, r.product_type as i16, trip_id, c.cancelled,
                    from_idx, to_mtime(c.departure.as_ref().unwrap().scheduled, timetable.start_time), if c.departure.as_ref().unwrap().is_live { Some(c.departure.as_ref().unwrap().delay_minutes as i16) } else { None },
                    to_idx, to_mtime(c.arrival.as_ref().unwrap().scheduled, timetable.start_time), if c.arrival.as_ref().unwrap().is_live { Some(c.arrival.as_ref().unwrap().delay_minutes as i16) } else { None }
                );
                if nc.product_type == 100 {
                    nc.departure.in_out_allowed = false; //TODO tstp footpaths not reachable, using virtual footpaths instead
                }
                nc.destination_arrival.replace(if !load_distributions || c.destination_arrival.is_none() { None } else { let da = c.destination_arrival.as_ref().unwrap(); Some(distribution::Distribution {
                    histogram: da.histogram.to_vec().into_iter().map(|h| h as types::MFloat).collect(),
                    start: to_mtime(da.start, timetable.start_time),
                    mean: (da.mean as types::MFloat/60.0) - timetable.start_time as types::MFloat,
                    feasible_probability: da.feasible_probability as types::MFloat
                }) });
                connections.push(nc);
                stations[from_idx].departures.push(id);
                stations[to_idx].arrivals.push(id);
            }
            trip_id += 1;
        }
        route_idx += 1;
    }
    for station in &mut *stations {
        station.departures.sort_unstable_by(|a,b| connections[*a].departure.projected().cmp(&connections[*b].departure.projected()));
    }
    let query = request_message.query.as_ref().unwrap();
    let start_time = request_message.timetable.as_ref().unwrap().start_time;

    let origin_id = query.origin.to_string();
    let destination_id = query.destination.to_string();
    let origin_idx = *stations_idx.get(&origin_id as &str).unwrap_or(&0);
    let destination_idx = *stations_idx.get(&destination_id as &str).unwrap_or(&0);
    
    let now = query.now;
    println!("orig {} dest {} stations {} connections {}", origin_id, destination_id, stations.len(), connections.len());
    QueryMetadata {
        start_ts: start_time,
        origin_id,
        origin_idx,
        destination_id,
        destination_idx,
        now,
        system: request_message.system.to_string()
    }
}

pub fn serialize_protobuf(stations: &[connection::Station], routes: &[connection::Route], connections: &[connection::Connection], _contraction: Option<&StationContraction>, metadata: &QueryMetadata) -> Vec<u8> {
    let mut wire_stations: Vec<wire::Station> = Vec::new();
    let mut trips: IndexMap<(i32, usize), Vec<wire::Connection>> = IndexMap::new();
    for s in stations.iter().enumerate() {
        wire_stations.push(wire::Station{
            id: Cow::Borrowed(&s.1.id),
            name: Cow::Borrowed(&s.1.name),
            lat: s.1.lat,
            lon: s.1.lon,
            parent: Cow::Owned(if s.1.parent_idx != 0 { s.1.parent_idx.to_string() } else { "".to_string() })
        });
    }
    let mut wire_routes: Vec<wire::Route> = routes.iter().map(|r| {
        wire::Route {
            id: Cow::Borrowed(&r.id),
            name: Cow::Borrowed(&r.name),
            product_type: r.product_type as i32,
            message: Cow::Borrowed(&r.message),
            direction: Cow::Borrowed(&r.direction),
            trips: vec![]
        }
    }).collect();
    for c in connections.iter().rev() {
        let mut route_idx = c.route_idx;
        if c.message == WALKING_MSG {
            route_idx = wire_routes.len();
            wire_routes.push(wire::Route{
                id: Cow::Owned(wire_routes.len().to_string()),
                name: Cow::Owned(walking::geodist_meters_string(&stations.get(c.from_idx).unwrap(), &stations.get(c.to_idx).unwrap())),
                product_type: WALKING_PRODUCT_TYPE as i32,
                message: Cow::Owned("".to_string()),
                direction: Cow::Owned("".to_string()),
                trips: vec![]
            });
        }
        if !trips.contains_key(&(c.trip_id, route_idx)) {
            trips.insert((c.trip_id, route_idx), vec![]);
        }
        let da = c.destination_arrival.borrow();
        trips.get_mut(&(c.trip_id, route_idx)).unwrap().push(wire::Connection{
            from_id: Cow::Borrowed(&stations.get(c.from_idx).unwrap().id),
            to_id: Cow::Borrowed(&stations.get(c.to_idx).unwrap().id),
            cancelled: false,
            departure: Some(wire::StopInfo{
                scheduled: from_mtime(c.departure.scheduled, metadata.start_ts),
                delay_minutes: c.departure.delay.unwrap_or(0) as i32,
                is_live: c.departure.delay.is_some(),
                scheduled_track: Cow::Borrowed(""),
                projected_track: Cow::Borrowed("")
            }),
            arrival: Some(wire::StopInfo{
                scheduled: from_mtime(c.arrival.scheduled, metadata.start_ts),
                delay_minutes: c.arrival.delay.unwrap_or(0) as i32,
                is_live: c.arrival.delay.is_some(),
                scheduled_track: Cow::Borrowed(""),
                projected_track: Cow::Borrowed("")
            }),
            message: Cow::Borrowed(""),
            destination_arrival: if da.is_none() || da.as_ref().unwrap().mean == 0.0 { None } else { let da = da.as_ref().unwrap(); Some(wire::Distribution {
                histogram: Cow::Owned(da.histogram.iter().map(|h| *h as f32).collect()),
                start: if da.start == 0 { 0 } else { from_mtime(da.start, metadata.start_ts) },
                mean: (da.mean*60.0) as i64 + metadata.start_ts,
                feasible_probability: da.feasible_probability as f32
            }) }
        });
    }
    for (key, connections) in trips.into_iter() {
        //connections.sort_by(|a, b| a.departure.as_ref().unwrap().scheduled.cmp(&b.departure.as_ref().unwrap().scheduled)); // maybe unnecessary
        wire_routes.get_mut(key.1).unwrap().trips.push(wire::Trip{
            connections: connections
        });
    }
    let response_message = wire::Message{
        timetable: Some(wire::Timetable{
            stations: wire_stations,
            routes: wire_routes,
            start_time: metadata.start_ts
        }),
        query: Some(wire::Query{
            origin: Cow::Borrowed(&stations[metadata.origin_idx].id),
            destination: Cow::Borrowed(&stations[metadata.destination_idx].id),
            now: 0
        }),
        system: Cow::Borrowed("")
    };
    let mut bytes = Vec::new();
    let mut writer = Writer::new(&mut bytes);
    let result = response_message.write_message(&mut writer);
    if result.is_err() {
        panic!("{:?}", result);
    }
    bytes
}