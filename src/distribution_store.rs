use std::cmp::Ordering;
use std::collections::HashMap;
use std::ops::Range;
use std::fs::File;
use csv;
use serde::{Serialize, Deserialize};
use rustc_hash::FxHashMap;
use itertools::Itertools;

use crate::distribution;
use crate::connection;
use crate::types;

const PRODUCT_TYPES_NUM: i16 = 13;

#[derive(PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Debug, Clone)]
struct DelayKey {
    product_type: i16,
    prior_delay: (i16, i16),
    prior_ttl: (i16, i16),
    is_departure: bool
}

#[derive(PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Debug)]
struct ReachabilityKey {
    from_product_type: i16,
    to_product_type: i16,
    from_prior_delay: (i16, i16),
    to_prior_delay: (i16, i16),
    from_prior_ttl: (i16, i16),
    to_prior_ttl: (i16, i16),
    diff: i16,
    from_is_departure: bool
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Store {
    delay: FxHashMap<DelayKey, distribution::Distribution>,
    delay_buckets: Vec<(i16, i16)>,
    delay_lower: i16,
    delay_upper: (i16, i16),
    ttl_buckets: Vec<(i16, i16)>,
    hot_ttl_buckets: Vec<i16>,
    hot_ttl_buckets_num: usize,
    ttl_lower: i16,
    reachability: FxHashMap<ReachabilityKey, f32>,
    hot_reachability: Vec<f32>,
    hot_reachability_factors: [usize; 5],
    min_delay_diff: i16,
    min_epsilon_delay_diff: i16,
    delay_range_size: usize,
    hits: usize,
    misses: usize
}

impl Store {
    pub fn new() -> Store {
        let mut s = Store{
            delay: FxHashMap::default(),
            delay_buckets: vec![],
            delay_lower: 0,
            delay_upper: (0,0),
            ttl_buckets: vec![],
            hot_ttl_buckets: vec![],
            hot_ttl_buckets_num: 0,
            ttl_lower: 0,
            reachability: FxHashMap::default(),
            hot_reachability: vec![],
            hot_reachability_factors: [0; 5],
            min_delay_diff: -180,
            min_epsilon_delay_diff: -180,
            delay_range_size: 0,
            hits: 0,
            misses: 0
        };
        s.insert_fallback_distributions();
        s
    }

    pub fn print_stats(&self) {
        println!("store: min_delay_diff: {} epsilon_min_delay_diff: {} reachability entries: {} hits: {} misses: {}", self.min_delay_diff, self.min_epsilon_delay_diff, self.reachability.len(), self.hits, self.misses);
    }

    pub fn reachability_len(&self) -> usize {
        self.reachability.len()
    }

    pub fn clear_reachability(&mut self) {
        self.reachability.clear();
    }

    #[inline(always)]
    fn delay_bucket(&self, delay: Option<i16>, ttl: (i16,i16)) -> (i16, i16) {
        if ttl == (0,0) {
            return (0,0)
        }
        match delay {
            Some(d) => if d >= self.delay_upper.0 { self.delay_upper } else { *self.delay_buckets.get((d-self.delay_lower) as usize).unwrap_or(&(0,0)) },
            None => (0,0)
        }        
    }

    #[inline(always)]
    fn ttl_bucket(&self, ttl: i32) -> (i16, i16) {
        *self.ttl_buckets.get((ttl-self.ttl_lower as i32) as usize).unwrap_or(&(0,0))
    }

    fn extend_bucket_mapping(bucket: (i16, i16), bucket_mapping: &mut Vec<(i16,i16)>, bucket_mapping_lower: &mut i16) {
        if bucket.0 < *bucket_mapping_lower {
            let mut prepended = vec![(0,0); (*bucket_mapping_lower-bucket.0) as usize];
            *bucket_mapping_lower = bucket.0;
            prepended.append(bucket_mapping);
            *bucket_mapping = prepended;
        }
        let required_len = (bucket.1 - *bucket_mapping_lower) as usize;
        if required_len > bucket_mapping.len() {
            bucket_mapping.resize(required_len, (0,0));
        }
    }

    fn insert_delay_key(&mut self, delay_key: DelayKey, distribution: distribution::Distribution) {
        let prior_delay_range = delay_key.prior_delay.0..delay_key.prior_delay.1;
        let prior_ttl_range = delay_key.prior_ttl.0..delay_key.prior_ttl.1;
        Self::extend_bucket_mapping(delay_key.prior_delay, &mut self.delay_buckets, &mut self.delay_lower);
        Self::extend_bucket_mapping(delay_key.prior_ttl, &mut self.ttl_buckets, &mut self.ttl_lower);
        for i in prior_delay_range {
            self.delay_buckets[(i-self.delay_lower) as usize] = delay_key.prior_delay;
        }
        for i in prior_ttl_range {
            self.ttl_buckets[(i-self.ttl_lower) as usize] = delay_key.prior_ttl;
        }
        if delay_key.prior_delay.0 >= self.delay_upper.0 {
            self.delay_upper = delay_key.prior_delay;
        }
        distribution.assert();
        self.delay.insert(delay_key, distribution);
    }

    pub fn insert_from_distribution(&mut self, prior_delay: Range<i16>, prior_ttl: Range<i16>, is_departure: bool, product_type: i16, distribution: distribution::Distribution) {
        let prior_delay_tuple = (prior_delay.start, prior_delay.end);
        let prior_ttl_tuple = (prior_ttl.start, prior_ttl.end);
        let delay_key = DelayKey{
            product_type: product_type,
            prior_delay: prior_delay_tuple,
            prior_ttl: prior_ttl_tuple,
            is_departure: is_departure
        };
        self.insert_delay_key(delay_key, distribution);
    }

    fn insert_distribution_from_buckets(&mut self, delay_key: DelayKey, latest_sample_delays: Vec<(Range<i16>, i32)>, total_feasible_sample_count: i32) -> Option<&distribution::Distribution> {
        if latest_sample_delays.len() == 0
            || latest_sample_delays.len() <= 3 && latest_sample_delays.iter().fold(0, |acc, l| l.0.end-l.0.start+acc) == 0
            || total_feasible_sample_count < 100 {
            //println!("Skipping {:?} {:?}", delay_key, latest_sample_delays);
            return None;
        }
        let d = distribution::Distribution::from_buckets(latest_sample_delays, total_feasible_sample_count);
        self.insert_delay_key(delay_key.clone(), d);
        Some(self.raw_delay_distribution_by_key(delay_key))
    }

    fn parse_bucket(bucket: &str) -> Range<i16> {
        if bucket == "NULL" {
            return 0..0;
        }
        let cleaned = bucket.replace('[', "").replace(')', "");
        let parts: Vec<&str> = cleaned.split(',').collect();
        let start = parts[0].parse();
        let end = parts[1].parse();
        start.clone().unwrap_or(end.clone().unwrap_or(0))..end.unwrap_or(start.unwrap_or(0))
    }

    fn make_delay_key(record: &csv::StringRecord, keys: &HashMap<&str, usize>) -> DelayKey {
        let prior_delay_bucket = Self::parse_bucket(record.get(keys["prior_delay_bucket"]).unwrap());
        let prior_ttl_bucket = Self::parse_bucket(record.get(keys["prior_ttl_bucket"]).unwrap());
        DelayKey {
            product_type: record.get(keys["product_type_id"]).unwrap().parse().unwrap(),
            prior_delay: (prior_delay_bucket.start, prior_delay_bucket.end),
            prior_ttl: (prior_ttl_bucket.start, prior_ttl_bucket.end),
            is_departure: record.get(keys["is_departure"]).unwrap() == "True"
        }
    }

    fn update_min_max_delay(dist: &distribution::Distribution, min_max_delay: &mut (i16, i16), epsilon_min_max_delay: &mut (i16, i16)) {
        if (dist.start as i16) < min_max_delay.0 {
            min_max_delay.0 = dist.start as i16;
        }
        if (dist.end() as i16) > min_max_delay.1 {
            min_max_delay.1 = dist.end() as i16;
        }
        let e = (0.01 as f32).sqrt();
        if (dist.quantile(e) as i16)-1 < epsilon_min_max_delay.0 {
            epsilon_min_max_delay.0 = dist.quantile(e) as i16-1;
        }
        if (dist.quantile(1.0-e) as i16) > epsilon_min_max_delay.1 {
            epsilon_min_max_delay.1 = dist.quantile(1.0-e) as i16;
        }
    }

    pub fn load_distributions(&mut self, file_path: &str) {
        let key_array: [(&str, usize); 6] = [
            ("product_type_id", 0),
            ("is_departure",1),
            ("prior_ttl_bucket",2),
            ("prior_delay_bucket",3),
            ("latest_sample_delay_bucket",4),
            ("sample_count",5)
        ];
        let keys = HashMap::from(key_array);
        let file = File::open(file_path).unwrap();
        let mut rdr = csv::Reader::from_reader(file);
        let mut current_delay_key: Option<DelayKey> = None;
        let mut latest_sample_delays: Vec<(Range<i16>, i32)> = vec![];
        let mut min_max_delay = (0,0);
        let mut epsilon_min_max_delay = (0,0);
        let mut total_feasible_sample_count = 0;
        for result in rdr.records() {
            let record = result.unwrap();
            let delay_key = Self::make_delay_key(&record, &keys);
            if current_delay_key.is_some() && delay_key != *current_delay_key.as_ref().unwrap() {
                let dist = self.insert_distribution_from_buckets(current_delay_key.unwrap(), latest_sample_delays, total_feasible_sample_count);
                if dist.is_some() {
                    Self::update_min_max_delay(dist.unwrap(), &mut min_max_delay, &mut epsilon_min_max_delay);
                }
                current_delay_key = None;
                latest_sample_delays = vec![];
                total_feasible_sample_count = 0;
            }
            current_delay_key.replace(delay_key);
            let latest_sample_delay = Self::parse_bucket(record.get(keys["latest_sample_delay_bucket"]).unwrap());
            let sample_count = record.get(keys["sample_count"]).unwrap().parse().unwrap();
            if latest_sample_delay.start != latest_sample_delay.end {
                total_feasible_sample_count += sample_count;
            }
            latest_sample_delays.push((latest_sample_delay, sample_count));
        }
        self.min_delay_diff = (min_max_delay.0-min_max_delay.1) as i16;
        self.min_epsilon_delay_diff = (epsilon_min_max_delay.0-epsilon_min_max_delay.1) as i16;
        self.delay_range_size = self.min_delay_diff.abs() as usize*2;
        self.create_hot_reachability();
    }

    fn create_hot_reachability(&mut self) {
        let map: HashMap<(i16, i16), i16> = HashMap::from_iter(self.ttl_buckets.iter().unique().map(|t| *t).sorted_by(|a,b| {
            if *a == (0,0) {
                Ordering::Less
            } else if *b == (0,0) {
                Ordering::Greater
            } else {
                a.cmp(&b)
            }
        }).enumerate().map(|t| (t.1, t.0 as i16+1)));
        println!("{:?}",map);
        self.hot_ttl_buckets_num = map.len()+1;
        self.hot_ttl_buckets = self.ttl_buckets.iter().map(|b| map[b]).collect();
        let len = 2*(PRODUCT_TYPES_NUM*PRODUCT_TYPES_NUM) as usize*self.hot_ttl_buckets_num*self.hot_ttl_buckets_num*self.delay_range_size;
        self.hot_reachability = vec![-1.0; len];
        self.hot_reachability_factors[0] = self.delay_range_size;
        self.hot_reachability_factors[1] = self.hot_reachability_factors[0]*PRODUCT_TYPES_NUM as usize;
        self.hot_reachability_factors[2] = self.hot_reachability_factors[1]*2;
        self.hot_reachability_factors[3] = self.hot_reachability_factors[2]*self.hot_ttl_buckets_num;
        self.hot_reachability_factors[4] = self.hot_reachability_factors[3]*self.hot_ttl_buckets_num;
    }

    fn insert_fallback_distributions(&mut self) {
        self.insert_delay_key(DelayKey {
            product_type: 100,
            prior_delay: (0,0),
            prior_ttl: (0,0),
            is_departure: true
        }, distribution::Distribution::uniform(0, 3));
        self.insert_delay_key(DelayKey {
            product_type: 100,
            prior_delay: (0,0),
            prior_ttl: (0,0),
            is_departure: false
        }, distribution::Distribution::uniform(-2, 3));
        self.insert_delay_key(DelayKey {
            product_type: -1,
            prior_delay: (0,0),
            prior_ttl: (0,0),
            is_departure: false
        }, distribution::Distribution::uniform(0, 1));
    }

    #[inline(always)]
    fn raw_delay_distribution(&self, delay_bucket: (i16,i16), is_departure: bool, product_type: i16, ttl_bucket: (i16, i16)) -> &distribution::Distribution {
        let key = DelayKey{
            product_type: product_type,
            prior_delay: delay_bucket,
            prior_ttl: ttl_bucket,
            is_departure: is_departure
        };
        self.raw_delay_distribution_by_key(key)
    }

    #[inline(always)]
    fn raw_delay_distribution_by_key(&self, key: DelayKey) -> &distribution::Distribution {
        match self.delay.get(&key) {
            Some(d) => d,
            None => {
                if key.product_type == 100 {
                    return self.delay.get(&DelayKey { product_type: 100, prior_delay: (0,0), prior_ttl: (0,0), is_departure: key.is_departure }).unwrap();
                }
                return self.delay.get(&DelayKey { product_type: -1, prior_delay: (0,0), prior_ttl: (0,0), is_departure: false }).unwrap();                
            }
        }
    }

    pub fn delay_distribution(&self, stop_info: &connection::StopInfo, is_departure: bool, product_type: i16, now: types::Mtime) -> distribution::Distribution {
        let ttl = self.ttl_bucket(stop_info.projected()-now);
        self.raw_delay_distribution(self.delay_bucket(stop_info.delay, ttl), is_departure, product_type, ttl).shift(stop_info.projected())
    }

    #[inline(always)]
    fn calculate_before_probability(&mut self, key: ReachabilityKey, from_prior_ttl: i32, to_prior_ttl: i32) -> f32 {
        let a = self.raw_delay_distribution(key.from_prior_delay, key.from_is_departure, key.from_product_type, key.from_prior_ttl);
        let d = self.raw_delay_distribution(key.to_prior_delay, true, key.to_product_type, key.to_prior_ttl);
        let mut p = a.before_probability(d, -key.diff as i32);
        if !key.from_is_departure {
            p *= d.feasible_probability;
        }
        if key.from_prior_delay == (0,0) && key.to_prior_delay == (0,0) && !self.hot_reachability.is_empty()
            && key.from_product_type < PRODUCT_TYPES_NUM
            && key.to_product_type < PRODUCT_TYPES_NUM  {
            let hot_idx = self.resolve_hot_reachability_index(key.diff, key.from_product_type, key.to_product_type, key.from_is_departure, from_prior_ttl, to_prior_ttl);
            self.hot_reachability[hot_idx] = p;
        } 
        self.reachability.insert(key, p);
        p
    }

    #[inline(always)]
    fn resolve_hot_ttl_bucket(&self, ttl: i32) -> usize {
        *self.hot_ttl_buckets.get((ttl-self.ttl_lower as i32) as usize).unwrap_or(&0) as usize
    }

    #[inline(always)]
    fn resolve_hot_reachability_index(&self, diff: i16, from_product_type: i16, to_product_type: i16, from_is_departure: bool, from_prior_ttl: i32, to_prior_ttl: i32) -> usize {
        (diff-self.min_delay_diff) as usize
        + to_product_type as usize*self.hot_reachability_factors[0]
        + from_is_departure as usize*self.hot_reachability_factors[1]
        + self.resolve_hot_ttl_bucket(to_prior_ttl)*self.hot_reachability_factors[2]
        + self.resolve_hot_ttl_bucket(from_prior_ttl)*self.hot_reachability_factors[3]
        + from_product_type as usize*self.hot_reachability_factors[4]
    }

    #[inline]
    pub fn before_probability(&mut self, from: &connection::StopInfo, from_product_type: i16, from_is_departure: bool, to: &connection::StopInfo, to_product_type: i16, transfer_time: i32, now: types::Mtime) -> f32 {
        let diff = (to.projected()-from.projected()-transfer_time) as i16;
        if diff < self.min_delay_diff {
            return 0.0;
        } else if diff > self.min_delay_diff.abs() {
            return 1.0;
        }
        let from_ttl = from.projected()-now;
        let to_ttl = to.projected()-now;
        if from.delay.is_none() && to.delay.is_none() && !self.hot_reachability.is_empty()
            && from_product_type < PRODUCT_TYPES_NUM
            && to_product_type < PRODUCT_TYPES_NUM {
            let p = self.hot_reachability[self.resolve_hot_reachability_index(diff, from_product_type, to_product_type, from_is_departure, from_ttl, to_ttl)];
            if p >= 0.0 {
                return p
            }
        }
        let from_ttl_bucket = self.ttl_bucket(from_ttl);
        let to_ttl_bucket = self.ttl_bucket(to_ttl);
        let key = ReachabilityKey{
            from_product_type,
            to_product_type,
            from_prior_delay: self.delay_bucket(from.delay, from_ttl_bucket),
            to_prior_delay: self.delay_bucket(to.delay, to_ttl_bucket),
            from_prior_ttl: from_ttl_bucket,
            to_prior_ttl: to_ttl_bucket,
            diff: diff,
            from_is_departure: from_is_departure
        };
        match self.reachability.get(&key) {
            Some(p) => {
                self.hits += 1;
                *p
            },
            None => {
                self.misses += 1;
                self.calculate_before_probability(key, from_ttl, to_ttl)
            }
        }
    }

    pub fn reachable_probability_conn(&mut self, arr: &connection::Connection, dep: &connection::Connection, now: types::Mtime) -> f32 {
        let p = self.before_probability(&arr.arrival, arr.product_type, false, &dep.departure, dep.product_type, 1, now);
        if arr.trip_id != dep.trip_id || arr.route_idx != dep.route_idx || arr.arrival.scheduled > dep.departure.scheduled {
            return p
        }
        if p < 0.04 {
            println!("unlikely safe transfer {} {:?} {:?} {:?} {:?} {:?}", p, arr.arrival, arr.product_type, arr.route_idx, dep.departure, dep.route_idx);
        }
        1.0
    }

    pub fn before_probability_conn(&mut self, before: &connection::Connection, after: &connection::Connection, now: types::Mtime) -> f32 {
        self.before_probability(&before.departure, before.product_type, true, &after.departure, after.product_type, 1, now)
    } 
}

#[cfg(test)]
mod tests {
    use super::*;
    fn setup() -> Store {
        let mut s = Store::new();
        s.insert_from_distribution(5..10, 30..45, true, 1, distribution::Distribution::uniform(-2, 3));
        s.insert_from_distribution(0..0, 30..45, true, 1, distribution::Distribution::uniform(-3, 4));
        s
    }

    #[test]
    fn insert() {
        let mut s = Store::new();
        s.insert_from_distribution(30..45, 10..15, true, 5, distribution::Distribution::uniform(55, 2));
        assert_eq!(s.delay_bucket(Some(33), (10,15)), (30,45));
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
    fn distribution_with_high_delay() {
        let s = setup();
        assert_eq!(s.delay_upper, (5,10));
        let d = s.delay_distribution(&connection::StopInfo{
            scheduled: 55,
            delay: Some(100),
            scheduled_track: "".to_string(),
            projected_track: "".to_string()
        }, true, 1, 120);
        assert_eq!(d.start, 153);
        assert_eq!(d.mean, 154.0);
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

    #[test]
    fn parse_bucket_normal() {
        let s = Store::parse_bucket("[5,10)");
        assert_eq!(s, 5..10);
    }

    #[test]
    fn parse_bucket_open_right() {
        let s = Store::parse_bucket("[5,)");
        assert_eq!(s, 5..5);
    }

    #[test]
    fn parse_bucket_open_left() {
        let s = Store::parse_bucket("(,10)");
        assert_eq!(s, 10..10);
    }

    #[test]
    fn parse_bucket_empty() {
        let s = Store::parse_bucket("(0,0)");
        assert_eq!(s, 0..0);
    }

    #[test]
    fn parse_bucket_null() {
        let s = Store::parse_bucket("NULL");
        assert_eq!(s, 0..0);
    }

    #[test]
    fn load_distributions_file() {
        let mut s = Store::new();
        s.load_distributions("./tests/fixtures/de_db.csv");
        //assert_eq!(s.delay.len(), 5830);
        assert_eq!(s.delay_upper, (91,91));
        //assert_eq!(s.delay_buckets.len(), 106);
        assert_eq!(s.ttl_buckets.len(), 380);
        let d = s.delay_distribution(&connection::StopInfo{
            scheduled: 55,
            delay: Some(65),
            scheduled_track: "".to_string(),
            projected_track: "".to_string()
        }, true, 4, 15);
        assert_eq!(d.start, 45);
        assert_float_absolute_eq!(d.mean, 133.21875);
        assert_eq!(d.histogram.len(), 136);
    }
   
}