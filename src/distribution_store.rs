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
    pub fn new() -> Store {
        Store{
            delay: HashMap::new(),
            delay_buckets: HashMap::new(),
            ttl_buckets: HashMap::new(),
            reachability: HashMap::new()
        }
    }

    fn delay_bucket(&self, delay: Option<i16>) -> (i16, i16) {
        match delay {
            Some(d) => *self.delay_buckets.get(&d).unwrap_or(&(0,0)),
            None => (0,0)
        }        
    }

    pub fn insert_distribution(&mut self, prior_delay: Range<i16>, prior_ttl: Range<i16>, is_departure: bool, product_type: i16, distribution: distribution::Distribution) {
        let prior_delay_tuple = (prior_delay.start, prior_delay.end);
        let prior_ttl_tuple = (prior_ttl.start, prior_ttl.end);
        self.delay.insert(DelayKey{
            product_type: product_type,
            prior_delay: prior_delay_tuple,
            prior_ttl: prior_ttl_tuple,
            is_departure: is_departure
        }, distribution);
        for i in prior_delay {
            self.delay_buckets.insert(i, prior_delay_tuple);
        }
        for i in prior_ttl {
            self.ttl_buckets.insert(i, prior_ttl_tuple);
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

    fn calculate_reachable_probability(&mut self, arrival: &connection::StopInfo, arrival_product_type: i16, departure: &connection::StopInfo, departure_product_type: i16, now: types::Mtime, key: ReachabilityKey) -> f32 {
        let a = self.delay_distribution(arrival, false, arrival_product_type, now);
        let d = self.delay_distribution(departure, true, departure_product_type, now);
        let p = a.before_probability(&d, 1)*d.feasible_probability;
        self.reachability.insert(key, p);
        p
    }

    pub fn reachable_probability(&mut self, arrival: &connection::StopInfo, arrival_product_type: i16, departure: &connection::StopInfo, departure_product_type: i16, now: types::Mtime) -> f32 {
        let key = ReachabilityKey{
            from_product_type: arrival_product_type,
            to_product_type: departure_product_type,
            from_prior_delay: self.delay_bucket(arrival.delay),
            to_prior_delay: self.delay_bucket(arrival.delay),
            prior_ttl: self.ttl_bucket((arrival.projected()-now) as i32),
            diff: (departure.projected()-arrival.projected()) as i16
        };
        match self.reachability.get(&key) {
            Some(p) => *p,
            None => self.calculate_reachable_probability(arrival, arrival_product_type, departure, departure_product_type, now, key)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn setup() -> Store {
        let mut s = Store::new();
        s.delay.insert(DelayKey{
            product_type: 1,
            prior_delay: (5,10),
            prior_ttl: (30,45),
            is_departure: true
        }, distribution::Distribution::uniform(-2, 3));
        s.delay.insert(DelayKey{
            product_type: 1,
            prior_delay: (0,0),
            prior_ttl: (30,45),
            is_departure: true
        }, distribution::Distribution::uniform(-3, 4));
        s.delay_buckets.insert(7, (5,10));
        s.ttl_buckets.insert(41, (30,45));
        s
    }

    #[test]
    fn insert() {
        let mut s = Store::new();
        s.insert_distribution(30..45, 10..15, true, 5, distribution::Distribution::uniform(55, 2));
        assert_eq!(s.delay_bucket(Some(33)), (30,45));
        assert_eq!(s.ttl_bucket(10), (10,15));
        assert_eq!(s.ttl_bucket(15), (0,0));
        let o = s.delay.get(&DelayKey{
            product_type: 5,
            prior_delay: (30,45),
            prior_ttl: (10,15),
            is_departure: true
        }).unwrap();
        assert_eq!(o.start, 55);
        assert_eq!(o.histogram.len(), 2);
    }

    #[test]
    fn distribution_with_delay() {
        let s = setup();
        let d = s.delay_distribution(&connection::StopInfo{
            scheduled: 55,
            delay: Some(7),
            scheduled_track: "".to_string(),
            projected_track: "".to_string()
        }, true, 1, 21);
        assert_eq!(d.start, 60);
        assert_eq!(d.mean, 61.0);
        assert_eq!(d.histogram.len(), 3);
    }

    #[test]
    fn distribution_with_nonexistant_delay() {
        let s = setup();
        let d = s.delay_distribution(&connection::StopInfo{
            scheduled: 55,
            delay: Some(1),
            scheduled_track: "".to_string(),
            projected_track: "".to_string()
        }, true, 1, 15);
        assert_eq!(d.start, 53);
        assert_eq!(d.mean, 54.5);
        assert_eq!(d.histogram.len(), 4);
    }

    #[test]
    fn distribution_with_no_delay() {
        let s = setup();
        let d = s.delay_distribution(&connection::StopInfo{
            scheduled: 55,
            delay: None,
            scheduled_track: "".to_string(),
            projected_track: "".to_string()
        }, true, 1, 14);
        assert_eq!(d.start, 52);
        assert_eq!(d.mean, 53.5);
        assert_eq!(d.histogram.len(), 4);
    }

    #[test]
    fn distribution_with_nonexistant_product() {
        let s = setup();
        let d = s.delay_distribution(&connection::StopInfo{
            scheduled: 55,
            delay: Some(1),
            scheduled_track: "".to_string(),
            projected_track: "".to_string()
        }, true, 555, 15);
        assert_eq!(d.start, 56);
        assert_eq!(d.mean, 56.0);
        assert_eq!(d.histogram.len(), 1);
    }
}