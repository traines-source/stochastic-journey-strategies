use std::io::Write;
use std::fs;
use std::borrow::Cow;
use std::borrow::Borrow;
use std::collections::HashMap;
use indexmap::IndexMap;


use quick_protobuf::{MessageRead, MessageWrite, BytesReader, Writer};

use crate::types;
use crate::wire::wire;
use crate::connection;
use crate::distribution;


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

pub fn deserialize_protobuf<'a, 'b>(bytes: Vec<u8>, stations: &'a mut Vec<connection::Station>, routes: &'b mut Vec<connection::Route>, connections: &'b mut Vec<connection::Connection>, load_distributions: bool) -> (i64, usize, usize, i64, String) {
    let mut reader = BytesReader::from_bytes(&bytes);
    let request_message = wire::Message::from_reader(&mut reader, &bytes).expect("Cannot read Timetable");
        
    let timetable = request_message.timetable.as_ref().unwrap();
    let mut stations_idx: HashMap<&str, usize> = HashMap::new(); 
    for s in &timetable.stations {
        stations_idx.insert(s.id.borrow(), stations.len());
        stations.push(connection::Station::new(s.id.to_string(), s.name.to_string(), vec![]));
    }
    let mut route_idx = 0;
    for r in &timetable.routes {
        routes.push(connection::Route::new(r.id.to_string(), r.name.to_string(), r.product_type as i16));
        let mut trip_id = 0;
        for t in &r.trips {
            for c in &t.connections {
                let from_idx = stations_idx[&c.from_id as &str];
                let to_idx = stations_idx[&c.to_id as &str];
                let id = connections.len();
                let nc = connection::Connection::new(
                    id, route_idx, r.product_type as i16, trip_id, c.cancelled,
                    from_idx, to_mtime(c.departure.as_ref().unwrap().scheduled, timetable.start_time), if c.departure.as_ref().unwrap().is_live { Some(c.departure.as_ref().unwrap().delay_minutes as i16) } else { None },
                    to_idx, to_mtime(c.arrival.as_ref().unwrap().scheduled, timetable.start_time), if c.arrival.as_ref().unwrap().is_live { Some(c.arrival.as_ref().unwrap().delay_minutes as i16) } else { None }
                );
                nc.destination_arrival.replace(if !load_distributions || c.destination_arrival.is_none() { None } else { let da = c.destination_arrival.as_ref().unwrap(); Some(distribution::Distribution {
                    histogram: da.histogram.to_vec(),
                    start: to_mtime(da.start, timetable.start_time),
                    mean: (da.mean as f32/60.0) - timetable.start_time as f32,
                    feasible_probability: da.feasible_probability
                }) });
                connections.push(nc);
                stations[from_idx].departures.push(id);
                stations[to_idx].arrivals.push(id);
            }
            trip_id += 1;
        }
        route_idx += 1;
    }
    let query = request_message.query.as_ref().unwrap();
    let start_time = request_message.timetable.as_ref().unwrap().start_time;

    let origin = query.origin.borrow() as &str;
    let destination = query.destination.borrow() as &str;
    let o = stations_idx[origin];
    let d = stations_idx[destination];
    let now = query.now;
    println!("orig {} dest {} stations {} connections {}", o, d, stations.len(), connections.len());
    (start_time, o, d, now, request_message.system.to_string())
}

pub fn serialize_protobuf(stations: &[connection::Station], routes: &[connection::Route], connections: &[connection::Connection], origin: &connection::Station, destination: &connection::Station, start_time: i64) -> Vec<u8> {
    let mut wire_stations: Vec<wire::Station> = Vec::new();
    let mut trips: IndexMap<usize, Vec<wire::Connection>> = IndexMap::new();
    for s in stations {
        wire_stations.push(wire::Station{
            id: Cow::Borrowed(&s.id),
            name: Cow::Borrowed(&s.name),
            lat: s.lat,
            lon: s.lon
        });
    }
    for c in connections {
        if !trips.contains_key(&c.route_idx) {
            trips.insert(c.route_idx, vec![]);
        }
        let da = c.destination_arrival.borrow();
        trips.get_mut(&c.route_idx).unwrap().push(wire::Connection{
            from_id: Cow::Borrowed(&stations.get(c.from_idx).unwrap().id),
            to_id: Cow::Borrowed(&stations.get(c.to_idx).unwrap().id),
            cancelled: false,
            departure: Some(wire::StopInfo{
                scheduled: from_mtime(c.departure.scheduled, start_time),
                delay_minutes: c.departure.delay.unwrap_or(0) as i32,
                is_live: c.departure.delay.is_some(),
                scheduled_track: Cow::Borrowed(""),
                projected_track: Cow::Borrowed("")
            }),
            arrival: Some(wire::StopInfo{
                scheduled: from_mtime(c.arrival.scheduled, start_time),
                delay_minutes: c.arrival.delay.unwrap_or(0) as i32,
                is_live: c.arrival.delay.is_some(),
                scheduled_track: Cow::Borrowed(""),
                projected_track: Cow::Borrowed("")
            }),
            message: Cow::Borrowed(""),
            destination_arrival: if da.is_none() { None } else { let da = da.as_ref().unwrap(); Some(wire::Distribution {
                histogram: Cow::Owned(da.histogram.clone()),
                start: from_mtime(da.start, start_time),
                mean: (da.mean*60.0) as i64 + start_time,
                feasible_probability: da.feasible_probability
            }) }
        });
    }
    let mut wire_routes = Vec::new();
    for (key, mut connections) in trips.into_iter() {
        connections.sort_by(|a, b| a.departure.as_ref().unwrap().scheduled.partial_cmp(&b.departure.as_ref().unwrap().scheduled).unwrap());
        wire_routes.push(wire::Route {
            id: Cow::Borrowed(&routes[key].id),
            name: Cow::Borrowed(""),
            product_type: 0,
            message: Cow::Borrowed(""),
            direction: Cow::Borrowed(""),
            trips: vec![wire::Trip{
                connections: connections
            }]
        });
    }

    let response_message = wire::Message{
        timetable: Some(wire::Timetable{
            stations: wire_stations,
            routes: wire_routes,
            start_time: start_time
        }),
        query: Some(wire::Query{
            origin: Cow::Borrowed(&origin.id),
            destination: Cow::Borrowed(&destination.id),
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