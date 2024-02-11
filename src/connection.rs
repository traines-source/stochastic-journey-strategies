use std::cell::RefCell;
use serde::{Serialize, Deserialize};

use crate::distribution;
use crate::types;

#[derive(Debug)]
pub struct Route {
	pub id: String,
	pub name: String,
	pub product_type: i16,
	pub message: String,
	pub direction: String
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

#[derive(Serialize, Deserialize, Debug)]
pub struct Station {
	pub id: String,
	pub name: String,
	pub arrivals: Vec<usize>,
	pub departures: Vec<usize>,
	pub lat: f32,
	pub lon: f32,
	pub transfer_time: u16,
	pub footpaths: Vec<motis_nigiri::Footpath>
}

impl<'a> Station {
	pub fn new(id: String, name: String, departures: Vec<usize>) -> Station {
		Station {
			id: id,
			name: name,
			arrivals: vec![],
			departures: departures,
			lat: 0.,
			lon: 0.,
			transfer_time: 0,
			footpaths: vec![]
		}
	}

	pub fn add_departure(&mut self, c_id: usize) {
		self.departures.push(c_id);
	}
}


#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Connection {
	pub id: usize,
	pub route_idx: usize,
	pub trip_id: i32,
	pub from_idx: usize,
	pub to_idx: usize,
	pub departure: StopInfo,
	pub arrival: StopInfo,
	pub message: String,
	pub cancelled: bool,
	pub product_type: i16,
	pub destination_arrival: RefCell<Option<distribution::Distribution>>	
}

impl<'a> Connection {
	pub fn new(id: usize, route_idx: usize, product_type: i16, trip_id: i32, cancelled: bool,
	from_idx: usize, from_scheduled: types::Mtime, from_delay: Option<i16>,
	to_idx: usize, to_scheduled: types::Mtime, to_delay: Option<i16>) -> Connection {
		Connection {
			id: id,
			route_idx: route_idx,
			trip_id: trip_id,
			from_idx: from_idx,
			to_idx: to_idx,
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
			cancelled: cancelled,
			product_type: product_type,
			destination_arrival: RefCell::new(None)
		}	
	}

	pub fn is_consecutive(&self, next: &Connection) -> bool {
		self.trip_id == next.trip_id && self.route_idx == next.route_idx && self.arrival.scheduled <= next.departure.scheduled && self.id != next.id && self.to_idx == next.from_idx
	}
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct StopInfo {
	pub scheduled: types::Mtime,
	pub delay: Option<i16>,
	pub scheduled_track: String,
	pub projected_track: String
}

impl StopInfo {
	pub fn new(scheduled: types::Mtime, delay: Option<i16>) -> StopInfo {
		StopInfo { scheduled: scheduled, delay: delay, scheduled_track: "".to_owned(), projected_track: "".to_owned() }
	}

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