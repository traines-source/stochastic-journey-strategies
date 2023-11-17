use crate::distribution;
use crate::types;

struct Route {
	id: String,
	name: String,
	product_type: i16,
	message: String,
	direction: String
}

pub struct Station<'a> {
	pub id: String,
	name: String,
	pub departures: &'a mut [Connection<'a>],
	lat: f32,
	lon: f32
}

pub struct Connection<'a> {
	route: &'a Route,
	pub from: &'a Station<'a>,
	pub to: &'a mut Station<'a>,
	pub departure: StopInfo,
	pub arrival: StopInfo,
	message: String,
	cancelled: bool,
	pub product_type: i16,
	pub destination_arrival: distribution::Distribution
}

pub struct StopInfo {
	pub scheduled: types::Mtime,
	pub delay: Option<i16>,
	scheduled_track: String,
	projected_track: String
}

impl StopInfo {
    pub fn projected(&self) -> types::Mtime {
        match self.delay {
            Some(d) => self.scheduled + d as types::Mtime,
            None => self.scheduled
        }
    }
}