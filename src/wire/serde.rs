use std::io::Write;
use std::fs;
use std::borrow::Cow;
use std::borrow::Borrow;
use std::collections::HashMap;

use quick_protobuf::{MessageRead, MessageWrite, BytesReader, Writer};

use crate::types;
use crate::wire::wire;
use crate::connection;


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

pub fn deserialize_protobuf<'a, 'b>(bytes: Vec<u8>, stations: &'a mut HashMap<String, connection::Station>, routes: &'a mut HashMap<String, connection::Route>, connections: &'b mut Vec<connection::Connection<'a>>) -> (i64, &'a connection::Station, &'a connection::Station, i64) {
    let mut reader = BytesReader::from_bytes(&bytes);
    let request_message = wire::Message::from_reader(&mut reader, &bytes).expect("Cannot read Timetable");
        
    let timetable = request_message.timetable.as_ref().unwrap();
    for s in &timetable.stations {
        stations.insert(s.id.to_string(), connection::Station::new(s.id.to_string(), s.name.to_string(), vec![]));
    }
    for r in &timetable.routes {
        routes.insert(r.id.to_string(), connection::Route::new(r.id.to_string(), r.name.to_string(), r.product_type as i16));
    }
        
    for r in &timetable.routes {
        let route = routes.get(&r.id.to_string()).unwrap();
        let mut trip_id = 0;
        for t in &r.trips {
            for c in &t.connections {
                let from = stations.get(c.from_id.borrow() as &str).unwrap();
                let to = stations.get(c.to_id.borrow() as &str).unwrap();
                let id = connections.len();
                connections.push(connection::Connection::new(
                    id, route, trip_id, c.cancelled,
                    from, to_mtime(c.departure.as_ref().unwrap().scheduled, timetable.start_time), if c.departure.as_ref().unwrap().is_live { Some(c.departure.as_ref().unwrap().delay_minutes as i16) } else { None },
                    to, to_mtime(c.arrival.as_ref().unwrap().scheduled, timetable.start_time), if c.arrival.as_ref().unwrap().is_live { Some(c.arrival.as_ref().unwrap().delay_minutes as i16) } else { None }
                ));
                from.departures.borrow_mut().push(id);
            }
            trip_id += 1;
        }        
    }
    let query = request_message.query.as_ref().unwrap();
    let start_time = request_message.timetable.as_ref().unwrap().start_time;

    let origin = query.origin.borrow() as &str;
    let destination = query.destination.borrow() as &str;
    let o = stations.get(origin).unwrap();
    let d = stations.get(destination).unwrap();
    let now = query.now;
    println!("orig {} dest {} stations {} connections {}", o.id, d.id, stations.len(), connections.len());
    (start_time, o, d, now)
}

pub fn serialize_protobuf(connections: &[connection::Connection], start_time: i64) -> Vec<u8> {
    let mut trips: HashMap<&str, Vec<wire::Connection>> = HashMap::new();
    for c in connections {
        if !trips.contains_key(&c.route.id as &str) {
            trips.insert(&c.route.id as &str, vec![]);
        }
        let da = c.destination_arrival.borrow();
        trips.get_mut(&c.route.id as &str).unwrap().push(wire::Connection{
            from_id: Cow::Borrowed(&c.from.id),
            to_id: Cow::Borrowed(&c.to.id),
            cancelled: false,
            departure: Some(wire::StopInfo{
                scheduled: from_mtime(c.departure.scheduled, start_time),
                delay_minutes: c.departure.delay.unwrap_or(0) as i32,
                is_live: c.departure.delay.is_some(),
                scheduled_track: Cow::Borrowed(""),
                projected_track: Cow::Borrowed("")
            }),
            arrival: None,
            message: Cow::Borrowed(""),
            destination_arrival: if da.is_none() { None } else { let da = da.as_ref().unwrap(); Some(wire::Distribution {
                histogram: Cow::Owned(da.histogram.clone()),
                start: from_mtime(da.start, start_time),
                mean: (da.mean*60.0) as i64 + start_time,
                feasible_probability: da.feasible_probability
            }) }
        })
    }
    let mut routes = Vec::new();
    for (key, mut connections) in trips {
        connections.sort_by(|a, b| a.departure.as_ref().unwrap().scheduled.partial_cmp(&b.departure.as_ref().unwrap().scheduled).unwrap());
        routes.push(wire::Route {
            id: Cow::Borrowed(key),
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
            stations: vec![],
            routes: routes,
            start_time: start_time
        }),
        query: None,
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