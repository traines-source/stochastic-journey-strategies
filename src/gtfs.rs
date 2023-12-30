use std::collections::HashMap;

use chrono;

use motis_nigiri::Timetable;

use crate::connection;

pub fn load<'a, 'b>(gtfs_path: &str, start_date: chrono::NaiveDate, end_date: chrono::NaiveDate, stations: &'a mut Vec<connection::Station>, routes: &'a mut Vec<connection::Route>, connections: &'b mut Vec<connection::Connection>) {
    let t = Timetable::load(gtfs_path, start_date, end_date);
    let gtfs_stops = t.get_stops();
    for s in gtfs_stops {
        stations.push(connection::Station::new(s.id.to_string(), s.name.to_string(), vec![]));
    }
    let gtfs_connections = t.get_connections();
    let mut i = 0;
    for c in gtfs_connections {
        let id = connections.len();
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
            id, c.route_idx.try_into().unwrap(), 30, c.trip_id.try_into().unwrap(), false,
            from_idx, c.departure.try_into().unwrap(), None,
            to_idx, c.arrival.try_into().unwrap(), None
        ));
        stations[from_idx].departures.borrow_mut().push(i);
        i += 1;
    }
}