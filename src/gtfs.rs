use motis_nigiri::Timetable;

use crate::connection;

pub fn load<'a, 'b>(stations: &'a mut Vec<connection::Station>, routes: &'a mut Vec<connection::Route>, connections: &'b mut Vec<connection::Connection>) {
    /*let t = Timetable::load();
    let gtfs_connections = t.get_connections();
    for c in gtfs_connections {
        let id = connections.len();
        connections.push(connection::Connection::new(
            id, c.route_idx, c.trip_id, false,
            c.from_idx, c.departure, None,
            c.to_idx, c.arrival, None
        ));
    }*/
}