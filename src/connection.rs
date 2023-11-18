use crate::distribution;
use crate::types;

pub struct Route {
	id: String,
	name: String,
	product_type: i16,
	message: String,
	direction: String
}

impl Route {
	pub fn new(id: String, name: String, product_type: i16) -> Route {
		Route {
			id: id,
			name: name,
			product_type: product_type,
			message: "".to_string(),
			direction: "".to_string()
		}
	}
}

pub struct Station<'a> {
	pub id: String,
	name: String,
	pub departures: &'a mut [Connection<'a>],
	lat: f32,
	lon: f32
}

impl<'a> Station<'_> {
	pub fn new(id: String, name: String, departures: &'a mut [Connection<'a>]) -> Station<'a> {
		Station {
			id: id,
			name: name,
			departures: departures,
			lat: 0.,
			lon: 0.
		}
	}
}

pub struct Connection<'a> {
	route: &'a Route,
	pub from: &'a Station<'a>,
	pub to: &'a mut Station<'a>,
	pub departure: StopInfo,
	pub arrival: StopInfo,
	message: String,
	cancelled_probability: f32,
	pub product_type: i16,
	pub destination_arrival: distribution::Distribution
}

impl<'a> Connection<'_> {
	pub fn new(route: &'a Route,
	from: &'a mut Station<'a>, from_scheduled: types::Mtime, from_delay: Option<i16>,
	to: &'a mut Station<'a>, to_scheduled: types::Mtime, to_delay: Option<i16>,
	cancelled_probability: f32) -> Connection<'a> {
		Connection {
			route: route,
			from: from,
			to: to,
			departure: StopInfo {
				scheduled: from_scheduled,
				delay: from_delay,
				scheduled_track: "".to_string(),
				projected_track: "".to_string()
			},
			arrival: StopInfo {
				scheduled: to_scheduled,
				delay: to_delay,
				scheduled_track: "".to_string(),
				projected_track: "".to_string()
			},
			message: "".to_string(),
			cancelled_probability: cancelled_probability,
			product_type: route.product_type,
			destination_arrival: distribution::Distribution::empty(0)
		}	
	}
}

pub struct StopInfo {
	pub scheduled: types::Mtime,
	pub delay: Option<i16>,
	pub scheduled_track: String,
	pub projected_track: String
}

impl StopInfo {
    pub fn projected(&self) -> types::Mtime {
        match self.delay {
            Some(d) => self.scheduled + d as types::Mtime,
            None => self.scheduled
        }
    }
}