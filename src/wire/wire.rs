// Automatically generated rust module for 'wire.proto' file

#![allow(non_snake_case)]
#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(unused_imports)]
#![allow(unknown_lints)]
#![allow(clippy::all)]
#![cfg_attr(rustfmt, rustfmt_skip)]


use std::borrow::Cow;
use quick_protobuf::{MessageInfo, MessageRead, MessageWrite, BytesReader, Writer, WriterBackend, Result};
use quick_protobuf::sizeofs::*;
use super::*;

#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct Distribution<'a> {
    pub histogram: Cow<'a, [f32]>,
    pub start: i64,
    pub mean: i64,
    pub feasible_probability: f32,
}

impl<'a> MessageRead<'a> for Distribution<'a> {
    fn from_reader(r: &mut BytesReader, bytes: &'a [u8]) -> Result<Self> {
        let mut msg = Self::default();
        while !r.is_eof() {
            match r.next_tag(bytes) {
                Ok(10) => msg.histogram = r.read_packed_fixed(bytes)?.into(),
                Ok(16) => msg.start = r.read_int64(bytes)?,
                Ok(24) => msg.mean = r.read_int64(bytes)?,
                Ok(37) => msg.feasible_probability = r.read_float(bytes)?,
                Ok(t) => { r.read_unknown(bytes, t)?; }
                Err(e) => return Err(e),
            }
        }
        Ok(msg)
    }
}

impl<'a> MessageWrite for Distribution<'a> {
    fn get_size(&self) -> usize {
        0
        + if self.histogram.is_empty() { 0 } else { 1 + sizeof_len(self.histogram.len() * 4) }
        + if self.start == 0i64 { 0 } else { 1 + sizeof_varint(*(&self.start) as u64) }
        + if self.mean == 0i64 { 0 } else { 1 + sizeof_varint(*(&self.mean) as u64) }
        + if self.feasible_probability == 0f32 { 0 } else { 1 + 4 }
    }

    fn write_message<W: WriterBackend>(&self, w: &mut Writer<W>) -> Result<()> {
        w.write_packed_fixed_with_tag(10, &self.histogram)?;
        if self.start != 0i64 { w.write_with_tag(16, |w| w.write_int64(*&self.start))?; }
        if self.mean != 0i64 { w.write_with_tag(24, |w| w.write_int64(*&self.mean))?; }
        if self.feasible_probability != 0f32 { w.write_with_tag(37, |w| w.write_float(*&self.feasible_probability))?; }
        Ok(())
    }
}

#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct StopInfo<'a> {
    pub scheduled: i64,
    pub delay_minutes: i32,
    pub is_live: bool,
    pub scheduled_track: Cow<'a, str>,
    pub projected_track: Cow<'a, str>,
}

impl<'a> MessageRead<'a> for StopInfo<'a> {
    fn from_reader(r: &mut BytesReader, bytes: &'a [u8]) -> Result<Self> {
        let mut msg = Self::default();
        while !r.is_eof() {
            match r.next_tag(bytes) {
                Ok(8) => msg.scheduled = r.read_int64(bytes)?,
                Ok(16) => msg.delay_minutes = r.read_sint32(bytes)?,
                Ok(24) => msg.is_live = r.read_bool(bytes)?,
                Ok(34) => msg.scheduled_track = r.read_string(bytes).map(Cow::Borrowed)?,
                Ok(42) => msg.projected_track = r.read_string(bytes).map(Cow::Borrowed)?,
                Ok(t) => { r.read_unknown(bytes, t)?; }
                Err(e) => return Err(e),
            }
        }
        Ok(msg)
    }
}

impl<'a> MessageWrite for StopInfo<'a> {
    fn get_size(&self) -> usize {
        0
        + if self.scheduled == 0i64 { 0 } else { 1 + sizeof_varint(*(&self.scheduled) as u64) }
        + if self.delay_minutes == 0i32 { 0 } else { 1 + sizeof_sint32(*(&self.delay_minutes)) }
        + if self.is_live == false { 0 } else { 1 + sizeof_varint(*(&self.is_live) as u64) }
        + if self.scheduled_track == "" { 0 } else { 1 + sizeof_len((&self.scheduled_track).len()) }
        + if self.projected_track == "" { 0 } else { 1 + sizeof_len((&self.projected_track).len()) }
    }

    fn write_message<W: WriterBackend>(&self, w: &mut Writer<W>) -> Result<()> {
        if self.scheduled != 0i64 { w.write_with_tag(8, |w| w.write_int64(*&self.scheduled))?; }
        if self.delay_minutes != 0i32 { w.write_with_tag(16, |w| w.write_sint32(*&self.delay_minutes))?; }
        if self.is_live != false { w.write_with_tag(24, |w| w.write_bool(*&self.is_live))?; }
        if self.scheduled_track != "" { w.write_with_tag(34, |w| w.write_string(&**&self.scheduled_track))?; }
        if self.projected_track != "" { w.write_with_tag(42, |w| w.write_string(&**&self.projected_track))?; }
        Ok(())
    }
}

#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct Connection<'a> {
    pub from_id: Cow<'a, str>,
    pub to_id: Cow<'a, str>,
    pub cancelled: bool,
    pub departure: Option<StopInfo<'a>>,
    pub arrival: Option<StopInfo<'a>>,
    pub message: Cow<'a, str>,
    pub destination_arrival: Option<Distribution<'a>>,
}

impl<'a> MessageRead<'a> for Connection<'a> {
    fn from_reader(r: &mut BytesReader, bytes: &'a [u8]) -> Result<Self> {
        let mut msg = Self::default();
        while !r.is_eof() {
            match r.next_tag(bytes) {
                Ok(10) => msg.from_id = r.read_string(bytes).map(Cow::Borrowed)?,
                Ok(18) => msg.to_id = r.read_string(bytes).map(Cow::Borrowed)?,
                Ok(24) => msg.cancelled = r.read_bool(bytes)?,
                Ok(34) => msg.departure = Some(r.read_message::<StopInfo>(bytes)?),
                Ok(42) => msg.arrival = Some(r.read_message::<StopInfo>(bytes)?),
                Ok(50) => msg.message = r.read_string(bytes).map(Cow::Borrowed)?,
                Ok(58) => msg.destination_arrival = Some(r.read_message::<Distribution>(bytes)?),
                Ok(t) => { r.read_unknown(bytes, t)?; }
                Err(e) => return Err(e),
            }
        }
        Ok(msg)
    }
}

impl<'a> MessageWrite for Connection<'a> {
    fn get_size(&self) -> usize {
        0
        + if self.from_id == "" { 0 } else { 1 + sizeof_len((&self.from_id).len()) }
        + if self.to_id == "" { 0 } else { 1 + sizeof_len((&self.to_id).len()) }
        + if self.cancelled == false { 0 } else { 1 + sizeof_varint(*(&self.cancelled) as u64) }
        + self.departure.as_ref().map_or(0, |m| 1 + sizeof_len((m).get_size()))
        + self.arrival.as_ref().map_or(0, |m| 1 + sizeof_len((m).get_size()))
        + if self.message == "" { 0 } else { 1 + sizeof_len((&self.message).len()) }
        + self.destination_arrival.as_ref().map_or(0, |m| 1 + sizeof_len((m).get_size()))
    }

    fn write_message<W: WriterBackend>(&self, w: &mut Writer<W>) -> Result<()> {
        if self.from_id != "" { w.write_with_tag(10, |w| w.write_string(&**&self.from_id))?; }
        if self.to_id != "" { w.write_with_tag(18, |w| w.write_string(&**&self.to_id))?; }
        if self.cancelled != false { w.write_with_tag(24, |w| w.write_bool(*&self.cancelled))?; }
        if let Some(ref s) = self.departure { w.write_with_tag(34, |w| w.write_message(s))?; }
        if let Some(ref s) = self.arrival { w.write_with_tag(42, |w| w.write_message(s))?; }
        if self.message != "" { w.write_with_tag(50, |w| w.write_string(&**&self.message))?; }
        if let Some(ref s) = self.destination_arrival { w.write_with_tag(58, |w| w.write_message(s))?; }
        Ok(())
    }
}

#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct Trip<'a> {
    pub connections: Vec<Connection<'a>>,
}

impl<'a> MessageRead<'a> for Trip<'a> {
    fn from_reader(r: &mut BytesReader, bytes: &'a [u8]) -> Result<Self> {
        let mut msg = Self::default();
        while !r.is_eof() {
            match r.next_tag(bytes) {
                Ok(10) => msg.connections.push(r.read_message::<Connection>(bytes)?),
                Ok(t) => { r.read_unknown(bytes, t)?; }
                Err(e) => return Err(e),
            }
        }
        Ok(msg)
    }
}

impl<'a> MessageWrite for Trip<'a> {
    fn get_size(&self) -> usize {
        0
        + self.connections.iter().map(|s| 1 + sizeof_len((s).get_size())).sum::<usize>()
    }

    fn write_message<W: WriterBackend>(&self, w: &mut Writer<W>) -> Result<()> {
        for s in &self.connections { w.write_with_tag(10, |w| w.write_message(s))?; }
        Ok(())
    }
}

#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct Route<'a> {
    pub id: Cow<'a, str>,
    pub name: Cow<'a, str>,
    pub product_type: i32,
    pub message: Cow<'a, str>,
    pub direction: Cow<'a, str>,
    pub trips: Vec<Trip<'a>>,
}

impl<'a> MessageRead<'a> for Route<'a> {
    fn from_reader(r: &mut BytesReader, bytes: &'a [u8]) -> Result<Self> {
        let mut msg = Self::default();
        while !r.is_eof() {
            match r.next_tag(bytes) {
                Ok(10) => msg.id = r.read_string(bytes).map(Cow::Borrowed)?,
                Ok(18) => msg.name = r.read_string(bytes).map(Cow::Borrowed)?,
                Ok(24) => msg.product_type = r.read_int32(bytes)?,
                Ok(34) => msg.message = r.read_string(bytes).map(Cow::Borrowed)?,
                Ok(42) => msg.direction = r.read_string(bytes).map(Cow::Borrowed)?,
                Ok(50) => msg.trips.push(r.read_message::<Trip>(bytes)?),
                Ok(t) => { r.read_unknown(bytes, t)?; }
                Err(e) => return Err(e),
            }
        }
        Ok(msg)
    }
}

impl<'a> MessageWrite for Route<'a> {
    fn get_size(&self) -> usize {
        0
        + if self.id == "" { 0 } else { 1 + sizeof_len((&self.id).len()) }
        + if self.name == "" { 0 } else { 1 + sizeof_len((&self.name).len()) }
        + if self.product_type == 0i32 { 0 } else { 1 + sizeof_varint(*(&self.product_type) as u64) }
        + if self.message == "" { 0 } else { 1 + sizeof_len((&self.message).len()) }
        + if self.direction == "" { 0 } else { 1 + sizeof_len((&self.direction).len()) }
        + self.trips.iter().map(|s| 1 + sizeof_len((s).get_size())).sum::<usize>()
    }

    fn write_message<W: WriterBackend>(&self, w: &mut Writer<W>) -> Result<()> {
        if self.id != "" { w.write_with_tag(10, |w| w.write_string(&**&self.id))?; }
        if self.name != "" { w.write_with_tag(18, |w| w.write_string(&**&self.name))?; }
        if self.product_type != 0i32 { w.write_with_tag(24, |w| w.write_int32(*&self.product_type))?; }
        if self.message != "" { w.write_with_tag(34, |w| w.write_string(&**&self.message))?; }
        if self.direction != "" { w.write_with_tag(42, |w| w.write_string(&**&self.direction))?; }
        for s in &self.trips { w.write_with_tag(50, |w| w.write_message(s))?; }
        Ok(())
    }
}

#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct Station<'a> {
    pub id: Cow<'a, str>,
    pub name: Cow<'a, str>,
    pub lat: f64,
    pub lon: f64,
    pub parent: Cow<'a, str>,
}

impl<'a> MessageRead<'a> for Station<'a> {
    fn from_reader(r: &mut BytesReader, bytes: &'a [u8]) -> Result<Self> {
        let mut msg = Self::default();
        while !r.is_eof() {
            match r.next_tag(bytes) {
                Ok(10) => msg.id = r.read_string(bytes).map(Cow::Borrowed)?,
                Ok(18) => msg.name = r.read_string(bytes).map(Cow::Borrowed)?,
                Ok(25) => msg.lat = r.read_double(bytes)?,
                Ok(33) => msg.lon = r.read_double(bytes)?,
                Ok(42) => msg.parent = r.read_string(bytes).map(Cow::Borrowed)?,
                Ok(t) => { r.read_unknown(bytes, t)?; }
                Err(e) => return Err(e),
            }
        }
        Ok(msg)
    }
}

impl<'a> MessageWrite for Station<'a> {
    fn get_size(&self) -> usize {
        0
        + if self.id == "" { 0 } else { 1 + sizeof_len((&self.id).len()) }
        + if self.name == "" { 0 } else { 1 + sizeof_len((&self.name).len()) }
        + if self.lat == 0f64 { 0 } else { 1 + 8 }
        + if self.lon == 0f64 { 0 } else { 1 + 8 }
        + if self.parent == "" { 0 } else { 1 + sizeof_len((&self.parent).len()) }
    }

    fn write_message<W: WriterBackend>(&self, w: &mut Writer<W>) -> Result<()> {
        if self.id != "" { w.write_with_tag(10, |w| w.write_string(&**&self.id))?; }
        if self.name != "" { w.write_with_tag(18, |w| w.write_string(&**&self.name))?; }
        if self.lat != 0f64 { w.write_with_tag(25, |w| w.write_double(*&self.lat))?; }
        if self.lon != 0f64 { w.write_with_tag(33, |w| w.write_double(*&self.lon))?; }
        if self.parent != "" { w.write_with_tag(42, |w| w.write_string(&**&self.parent))?; }
        Ok(())
    }
}

#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct Timetable<'a> {
    pub stations: Vec<Station<'a>>,
    pub routes: Vec<Route<'a>>,
    pub start_time: i64,
}

impl<'a> MessageRead<'a> for Timetable<'a> {
    fn from_reader(r: &mut BytesReader, bytes: &'a [u8]) -> Result<Self> {
        let mut msg = Self::default();
        while !r.is_eof() {
            match r.next_tag(bytes) {
                Ok(10) => msg.stations.push(r.read_message::<Station>(bytes)?),
                Ok(18) => msg.routes.push(r.read_message::<Route>(bytes)?),
                Ok(24) => msg.start_time = r.read_int64(bytes)?,
                Ok(t) => { r.read_unknown(bytes, t)?; }
                Err(e) => return Err(e),
            }
        }
        Ok(msg)
    }
}

impl<'a> MessageWrite for Timetable<'a> {
    fn get_size(&self) -> usize {
        0
        + self.stations.iter().map(|s| 1 + sizeof_len((s).get_size())).sum::<usize>()
        + self.routes.iter().map(|s| 1 + sizeof_len((s).get_size())).sum::<usize>()
        + if self.start_time == 0i64 { 0 } else { 1 + sizeof_varint(*(&self.start_time) as u64) }
    }

    fn write_message<W: WriterBackend>(&self, w: &mut Writer<W>) -> Result<()> {
        for s in &self.stations { w.write_with_tag(10, |w| w.write_message(s))?; }
        for s in &self.routes { w.write_with_tag(18, |w| w.write_message(s))?; }
        if self.start_time != 0i64 { w.write_with_tag(24, |w| w.write_int64(*&self.start_time))?; }
        Ok(())
    }
}

#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct Query<'a> {
    pub origin: Cow<'a, str>,
    pub destination: Cow<'a, str>,
    pub now: i64,
}

impl<'a> MessageRead<'a> for Query<'a> {
    fn from_reader(r: &mut BytesReader, bytes: &'a [u8]) -> Result<Self> {
        let mut msg = Self::default();
        while !r.is_eof() {
            match r.next_tag(bytes) {
                Ok(10) => msg.origin = r.read_string(bytes).map(Cow::Borrowed)?,
                Ok(18) => msg.destination = r.read_string(bytes).map(Cow::Borrowed)?,
                Ok(24) => msg.now = r.read_int64(bytes)?,
                Ok(t) => { r.read_unknown(bytes, t)?; }
                Err(e) => return Err(e),
            }
        }
        Ok(msg)
    }
}

impl<'a> MessageWrite for Query<'a> {
    fn get_size(&self) -> usize {
        0
        + if self.origin == "" { 0 } else { 1 + sizeof_len((&self.origin).len()) }
        + if self.destination == "" { 0 } else { 1 + sizeof_len((&self.destination).len()) }
        + if self.now == 0i64 { 0 } else { 1 + sizeof_varint(*(&self.now) as u64) }
    }

    fn write_message<W: WriterBackend>(&self, w: &mut Writer<W>) -> Result<()> {
        if self.origin != "" { w.write_with_tag(10, |w| w.write_string(&**&self.origin))?; }
        if self.destination != "" { w.write_with_tag(18, |w| w.write_string(&**&self.destination))?; }
        if self.now != 0i64 { w.write_with_tag(24, |w| w.write_int64(*&self.now))?; }
        Ok(())
    }
}

#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct Message<'a> {
    pub timetable: Option<Timetable<'a>>,
    pub query: Option<Query<'a>>,
    pub system: Cow<'a, str>,
}

impl<'a> MessageRead<'a> for Message<'a> {
    fn from_reader(r: &mut BytesReader, bytes: &'a [u8]) -> Result<Self> {
        let mut msg = Self::default();
        while !r.is_eof() {
            match r.next_tag(bytes) {
                Ok(10) => msg.timetable = Some(r.read_message::<Timetable>(bytes)?),
                Ok(18) => msg.query = Some(r.read_message::<Query>(bytes)?),
                Ok(26) => msg.system = r.read_string(bytes).map(Cow::Borrowed)?,
                Ok(t) => { r.read_unknown(bytes, t)?; }
                Err(e) => return Err(e),
            }
        }
        Ok(msg)
    }
}

impl<'a> MessageWrite for Message<'a> {
    fn get_size(&self) -> usize {
        0
        + self.timetable.as_ref().map_or(0, |m| 1 + sizeof_len((m).get_size()))
        + self.query.as_ref().map_or(0, |m| 1 + sizeof_len((m).get_size()))
        + if self.system == "" { 0 } else { 1 + sizeof_len((&self.system).len()) }
    }

    fn write_message<W: WriterBackend>(&self, w: &mut Writer<W>) -> Result<()> {
        if let Some(ref s) = self.timetable { w.write_with_tag(10, |w| w.write_message(s))?; }
        if let Some(ref s) = self.query { w.write_with_tag(18, |w| w.write_message(s))?; }
        if self.system != "" { w.write_with_tag(26, |w| w.write_string(&**&self.system))?; }
        Ok(())
    }
}

