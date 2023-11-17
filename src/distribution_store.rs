use std::collections::HashMap;
use std::ops::Range;
use crate::distribution;
use crate::connection;
use crate::types;


#[derive(PartialEq, Eq, PartialOrd, Ord, Hash)]
struct DelayKey {
    product_type: i16,
    prior_delay: (i16, i16),
    prior_ttl: (i16, i16),
    is_departure: bool
}

#[derive(PartialEq, Eq, PartialOrd, Ord, Hash)]
struct ReachabilityKey {
    from_product_type: i16,
    to_product_type: i16,
    from_prior_delay: (i16, i16),
    to_prior_delay: (i16, i16),
    prior_ttl: (i16, i16),
    diff: i16
}
pub struct Store {
    delay: HashMap<DelayKey, distribution::Distribution>,
    delay_buckets: HashMap<i16, (i16, i16)>,
    ttl_buckets: HashMap<i16, (i16, i16)>,
    reachability: HashMap<ReachabilityKey, f32>
}

impl Store {
    fn delay_bucket(&self, delay: Option<i16>) -> (i16, i16) {
        match delay {
            Some(d) => *self.delay_buckets.get(&d).unwrap_or(&(0,0)),
            None => (0,0)
        }        
    }

    fn ttl_bucket(&self, ttl: i32) -> (i16, i16) {
        if ttl > 360 {
            return (360,361)
        } else if ttl < -15 {
            return (-15,-15)
        }
        *self.ttl_buckets.get(&(ttl as i16)).unwrap_or(&(0,0))
    }

    pub fn delay_distribution(&self, stop_info: &connection::StopInfo, is_departure: bool, product_type: i16, now: types::Mtime) -> distribution::Distribution {
        match self.delay.get(&DelayKey{
            product_type: product_type,
            prior_delay: self.delay_bucket(stop_info.delay),
            prior_ttl: self.ttl_bucket((stop_info.projected()-now) as i32),
            is_departure: is_departure
        }) {
            Some(d) => d.shift(stop_info.projected()),
            None => distribution::Distribution::uniform(stop_info.projected(), 1)
        }
    }

    pub fn reachable_probability(&self, arrival: &connection::StopInfo, arrival_product_type: i16, departure: &connection::StopInfo, departure_product_type: i16, now: types::Mtime) -> f32 {
        1.0
    }
}