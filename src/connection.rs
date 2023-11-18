use std::cell::RefCell;

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
	pub departures: RefCell<Vec<&'a Connection<'a>>>,
	lat: f32,
	lon: f32
}

impl<'a> Station<'a> {
	pub fn new(id: String, name: String, departures: Vec<&'a Connection<'a>>) -> Station<'a> {
		Station {
			id: id,
			name: name,
			departures: RefCell::new(departures),
			lat: 0.,
			lon: 0.
		}
	}

	pub fn add_departure(&self, c: &'a Connection<'a>) {
		self.departures.borrow_mut().push(c);
	}
}

pub struct Connection<'a> {
	route: &'a Route,
	pub from: &'a Station<'a>,
	pub to: &'a Station<'a>,
	pub departure: StopInfo,
	pub arrival: StopInfo,
	message: String,
	pub cancelled_probability: f32,
	pub product_type: i16,
	pub destination_arrival: RefCell<distribution::Distribution>
}

impl<'a> Connection<'a> {
	pub fn new(route: &'a Route,
	from: &'a Station<'a>, from_scheduled: types::Mtime, from_delay: Option<i16>,
	to: &'a Station<'a>, to_scheduled: types::Mtime, to_delay: Option<i16>,
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
			destination_arrival: RefCell::new(distribution::Distribution::empty(0))
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


#[cfg(test)]
mod tests {
	use super::*;

    #[test]
    fn projected_delay() {
        let s = StopInfo{
			scheduled: 5,
			delay: Some(3),
			scheduled_track: "".to_string(),
			projected_track: "".to_string()
		};
		assert_eq!(s.projected(), 8);
    }
	#[test]
	fn projected_wo_delay() {
        let s = StopInfo{
			scheduled: 5,
			delay: None,
			scheduled_track: "".to_string(),
			projected_track: "".to_string()
		};
		assert_eq!(s.projected(), 5);
    }
}