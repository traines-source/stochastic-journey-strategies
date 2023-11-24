use stost::wire::wire;
use stost::distribution_store;
use stost::query;
use stost::connection;
use stost::types;
use quick_protobuf::{MessageRead, MessageWrite, BytesReader, Writer};

use rouille::Response;
use std::io::Read;
use std::borrow::Cow;
use std::borrow::Borrow;
use std::collections::HashMap;
use std::sync::Mutex;

fn to_mtime(t: i64, reference: i64) -> types::Mtime {
    ((t-reference) as f32/60.0).round() as types::Mtime
}

fn from_mtime(mtime: types::Mtime, reference: i64) -> i64 {
    (mtime*60) as i64 + reference
}


fn main() {
    let mut store = distribution_store::Store::new();
    store.load_distributions("./data/de_db.csv");
    let store_mutex = Mutex::new(store);

    println!("starting...");
    rouille::start_server("0.0.0.0:1234", move |request| {

        println!("receiving req...");
        let mut bytes: Vec<u8> = vec![];
        request.data().unwrap().read_to_end(&mut bytes);
        let mut reader = BytesReader::from_bytes(&bytes);
        let request_message = wire::Message::from_reader(&mut reader, &bytes).expect("Cannot read Timetable");

        let mut stations: HashMap<&str, connection::Station> = HashMap::new();
        let mut routes = HashMap::new();
        let timetable = request_message.timetable.as_ref().unwrap();
        for s in &timetable.stations {
            stations.insert(&s.id, connection::Station::new(s.id.to_string(), s.name.to_string(), vec![]));
        }
        for r in &timetable.routes {
            routes.insert(&r.id, connection::Route::new(r.id.to_string(), r.name.to_string(), r.product_type as i16));
        }
        let mut connections = 0;
        for r in &timetable.routes {
            let route = routes.get(&r.id).unwrap();
            let mut trip_id = 0;
            for t in &r.trips {
                for c in &t.connections {
                    let from = stations.get(c.from_id.borrow() as &str).unwrap();
                    let to = stations.get(c.to_id.borrow() as &str).unwrap();
                    from.departures.borrow_mut().push(connection::Connection::new(
                        route, trip_id, c.cancelled,
                        from, to_mtime(c.departure.as_ref().unwrap().scheduled, timetable.start_time), if c.departure.as_ref().unwrap().is_live { Some(c.departure.as_ref().unwrap().delay_minutes as i16) } else { None },
                        to, to_mtime(c.arrival.as_ref().unwrap().scheduled, timetable.start_time), if c.arrival.as_ref().unwrap().is_live { Some(c.arrival.as_ref().unwrap().delay_minutes as i16) } else { None }
                    ));
                    connections += 1;
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
        println!("orig {} dest {} stations {} connections {}", o.id, d.id, stations.len(), connections);
        let mut s = store_mutex.lock().unwrap();
        println!("querying...");       
        query::query(&mut s, o, d, 0, 100, to_mtime(query.now, start_time));
        println!("finished querying.");
        let mut trips: HashMap<&str, Vec<wire::Connection>> = HashMap::new();
        for (_, s) in &stations {
            for c in &*s.departures.borrow() {
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
                    destination_arrival: if (da.is_none()) { None } else { let da = da.as_ref().unwrap(); Some(wire::Distribution {
                        histogram: Cow::Owned(da.histogram.clone()),
                        start: from_mtime(da.start, start_time),
                        mean: (da.mean*60.0) as i64 + start_time,
                        feasible_probability: da.feasible_probability
                    }) }
                })
            }
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
        response_message.write_message(&mut writer);        
        Response::from_data("application/octet-stream", bytes)
    });
}