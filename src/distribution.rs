use std::cmp;
use std::ops::Range;
use serde::{Serialize, Deserialize};

use crate::types;
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Distribution {
    #[serde(skip_deserializing)]
    pub histogram: Vec<types::MFloat>,
	pub start: types::Mtime,
    #[serde(default)]
	pub mean: types::MFloat,
    #[serde(default)]
    pub feasible_probability: types::MFloat
}

const EMPTY_HISTOGRAM: Vec<types::MFloat> = vec![];

impl Distribution {

    #[inline(always)]
	pub fn exists(&self) -> bool {
        self.mean != 0.0 || self.histogram.len() > 0
    }

    pub fn assert(&self) {
        assert_float_absolute_eq!(self.histogram.iter().sum::<types::MFloat>(), 1.0, 1e-3);
    }

    pub fn end(&self) -> types::Mtime {
        self.start+self.histogram.len() as types::Mtime
    }

    #[inline(always)]
    pub fn empty(start: types::Mtime) -> Distribution {
        Distribution{
            histogram: EMPTY_HISTOGRAM,
            start: start,
            mean: 0.,
            feasible_probability: 0.0
        }
    }

    pub fn uniform(start: types::Mtime, width: usize) -> Distribution {
        if width == 0 {
            return Distribution::empty(start);
        }
        Distribution{
            histogram: vec![1.0/(width as types::MFloat); width],
            start: start,
            mean:  start as types::MFloat+((width-1) as types::MFloat/2.0),
            feasible_probability: 1.0
        }
    }

    pub fn mean(&self) -> types::MFloat {
        let mut mean = 0.0;
        for i in 0..self.histogram.len() {
            mean += (self.start as types::MFloat+i as types::MFloat)*self.histogram[i];
        }
        mean as types::MFloat
    }

    pub fn quantile(&self, q: types::MFloat) -> types::Mtime {
        let mut cum = 0.0;
        for i in 0..self.histogram.len() {
            cum += self.histogram[i];
            if cum >= q {
                return self.start+i as types::Mtime;
            }
        }
        return self.end();
    }

    pub fn normalize(&mut self) {
        self.normalize_with(false, 0.0);    
    }
    
    #[inline]
    pub fn normalize_with(&mut self, mean_only: bool, epsilon: types::MFloat) {
        // TODO performance vs accuracy
        if self.feasible_probability == 0.0 {
            return;
        }
        if !mean_only {
            if self.histogram.len() == 0 {
                return;
            }

            let mut sum = 0.0;
            let mut last = 0;
            let mut offset = 0;
            let mut found = false;
            for i in 0..self.histogram.len() {
                if self.histogram[i] > epsilon {
                    if !found {
                        offset = i;
                        self.start += i as i32;
                        found = true;
                    }
                    last = i;
                    sum += self.histogram[i];
                }
            }
            if sum > 0.0 {
                let new_len = last-offset+1;
                for i in 0..new_len {
                    self.histogram[i] = self.histogram[i+offset]/sum;
                }
                self.histogram.truncate(new_len);
            }
        }
        self.mean /= self.feasible_probability;
    }
    
    pub fn add(&mut self, other: &Distribution, weight: types::MFloat) {
        self.add_with(other, weight, false);    
    }

    #[inline]
    pub fn add_with(&mut self, other: &Distribution, weight: types::MFloat, mean_only: bool) {
        if mean_only {
            self.mean += other.mean*weight;
            return;
        }
        if !self.exists() {
            self.start = other.start;
        }
        let self_start = self.start;
        let other_start = other.start;
        let start = cmp::min(self_start, other_start);
        let end = cmp::max(self_start+self.histogram.len() as i32, other_start+other.histogram.len() as i32);
        let self_offset = (self_start-start) as usize;
        let other_offset = (other_start-start) as usize;
        let new_len = (end-start) as usize;
        let mut h = vec![0.; new_len];

        for i in 0..new_len {
            if i >= self_offset && i-self_offset < self.histogram.len() {
                h[i] += self.histogram[i-self_offset];
            }
            if i >= other_offset && i-other_offset < other.histogram.len() {
                h[i] += other.histogram[i-other_offset]*weight as types::MFloat;
            }
        }
        self.histogram = h;
		self.start = start as types::Mtime;
        self.mean += other.mean*weight;
    }

    pub fn shift(&self, start: types::Mtime) -> Distribution {
        Distribution{
            histogram: self.histogram.clone(),
            start: self.start+start,
            mean: self.mean+start as types::MFloat,
            feasible_probability: self.feasible_probability
        }
    }

    pub fn before_probability(&self, other: &Distribution, offset: i32) -> types::MFloat {
        let self_len = self.histogram.len() as i32;
        let other_len = other.histogram.len() as i32;
        let diff = other.start-self.start-offset;
        if diff+other_len <= 0 {
            return 0.0;
        }
        if self_len < diff {
            return 1.0;
        }
        let mut cumulative = 0.0;
        let until_other_start = std::cmp::min(diff, self_len);
        for i in 0..until_other_start {
            cumulative += self.histogram[i as usize];
        }
        let mut p = 0.0;
        for j in 0..other_len {
            let i = diff+j;
            if i < 0 {
                continue
            }
            if i < self_len {
                cumulative += self.histogram[i as usize];
            }
            p += cumulative*other.histogram[j as usize];
        }
        if p > 1.0 {
            return 1.0
        }
        p
    }

    pub fn from_buckets(latest_sample_delays: Vec<(Range<i16>, i32)>, total_feasible_sample_count: i32) -> Distribution {
        let total = total_feasible_sample_count as types::MFloat;
        let cancelled = 0..0;
        let mut h = vec![];
        let mut feasibility = 1.0;
        let mut mean = 0.0;
        let mut sum = 0;
        let mut i = latest_sample_delays[0].0.start;
        for bucket in &latest_sample_delays {
            if bucket.0 == cancelled {
                feasibility = total/(total+bucket.1 as types::MFloat);
                continue;
            }
            while i < bucket.0.start {
                h.push(0.0);
                i += 1;
            }      
            let len = bucket.0.len() as types::MFloat;      
            for _j in bucket.0.clone() {
                let fraction = bucket.1 as types::MFloat/total/len;
                h.push(fraction);
                mean += i as types::MFloat*fraction;
                i += 1;
            }
            if bucket.0.len() > 0 {
                sum += bucket.1;
            }
        }
        assert_eq!(sum, total_feasible_sample_count);
        assert_float_absolute_eq!(h.iter().sum::<types::MFloat>(), 1.0, 1e-3);
        let d = Distribution{
            histogram: h,
            start: latest_sample_delays[0].0.start as i32,
            mean:  mean as types::MFloat,
            feasible_probability: feasibility as types::MFloat
        };
        assert_float_absolute_eq!(mean as types::MFloat, d.mean());
        d
    }

    pub fn nonnegative(&mut self) {
        if self.start < 0 {
            let until0 = cmp::min(self.histogram.len(), (-self.start+1) as usize);
            self.histogram.splice(0..until0, std::iter::once(self.histogram.iter().take(until0).sum()));
            self.start = 0;
            self.mean = self.mean();
            assert_float_absolute_eq!(1.0, self.histogram.iter().sum::<f32>(), 1e-3);
        }
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty() {
        let a = Distribution::empty(0);
        assert_eq!(a.histogram.len(), 0);
        assert_eq!(a.start, 0);
        assert_eq!(a.mean, 0.);
    }

    #[test]
    fn uniform_empty() {
        let a = Distribution::uniform(0, 0);
        assert_eq!(a.histogram.len(), 0);
        assert_eq!(a.start, 0);
        assert_eq!(a.mean, 0.);
    }

    #[test]
    fn uniform_one() {
        let a = Distribution::uniform(5, 1);
        assert_eq!(a.histogram.len(), 1);
        assert_eq!(a.histogram[0], 1.0);
        assert_eq!(a.start, 5);
        assert_eq!(a.mean, 5.);
    }

    #[test]
    fn uniform_four() {
        let a = Distribution::uniform(2, 4);
        assert_eq!(a.histogram.len(), 4);
        assert_eq!(a.histogram[0], 0.25);
        assert_eq!(a.histogram[1], 0.25);
        assert_eq!(a.histogram[2], 0.25);
        assert_eq!(a.histogram[3], 0.25);
        assert_eq!(a.start, 2);
        assert_eq!(a.mean, a.mean());
        assert_eq!(a.mean, 3.5);
    }

    #[test]
    fn mean_negative() {
        assert_eq!(Distribution::uniform(-2, 4).mean(), -0.5);
    }

    #[test]
    fn add_empty() {
        let mut a = Distribution::empty(0);
        let b = Distribution::empty(0);
        a.add(&b, 1.0);
        assert_eq!(a.histogram.len(), 0);
        assert_eq!(a.start, 0);
        assert_eq!(a.mean, a.mean());
        assert_eq!(a.exists(), false);
    }

    #[test]
    fn add_empty_apart() {
        let mut a = Distribution::empty(10);
        let b = Distribution::empty(5);
        a.add(&b, 1.0);
        assert_eq!(a.histogram.len(), 0);
        assert_eq!(a.start, 5);
        assert_eq!(a.mean, a.mean());
        assert_eq!(a.exists(), false);
    }

    #[test]
    fn add_uniform_overlapping() {
        let mut a = Distribution::uniform(5, 2);
        let b = Distribution::uniform(6, 4);
        a.add(&b, 0.5);
        assert_eq!(a.histogram.len(), 5);
        assert_eq!(a.start, 5);
        assert_eq!(a.histogram[0], 0.5);
        assert_eq!(a.histogram[1], 0.625);
        assert_eq!(a.histogram[2], 0.125);
        assert_eq!(a.histogram[3], 0.125);
        assert_eq!(a.histogram[4], 0.125);
        assert_eq!(a.mean, 9.25);
        assert_eq!(a.mean, a.mean());
    }

    #[test]
    fn add_uniform_apart() {
        let mut a = Distribution::uniform(5, 2);
        let b = Distribution::uniform(8, 2);
        a.add(&b, 0.5);
        assert_eq!(a.histogram.len(), 5);
        assert_eq!(a.start, 5);
        assert_eq!(a.histogram[0], 0.5);
        assert_eq!(a.histogram[1], 0.5);
        assert_eq!(a.histogram[2], 0.);
        assert_eq!(a.histogram[3], 0.25);
        assert_eq!(a.histogram[4], 0.25);
    }

    #[test]
    fn add_negative() {
        let mut a = Distribution::uniform(0, 1);
        let b = Distribution::uniform(-4, 1);
        a.add(&b, 1.0);
        assert_eq!(a.histogram.len(), 5);
        assert_eq!(a.start, -4);
        assert_eq!(a.histogram[0], 1.0);
        assert_eq!(a.histogram[4], 1.0);
        assert_eq!(a.mean, a.mean());
        assert_eq!(a.exists(), true);
    }

    #[test]
    fn add_two_negative() {
        let mut a = Distribution::uniform(-5, 2);
        let b = Distribution::uniform(-4, 1);
        a.add(&b, 1.0);
        assert_eq!(a.histogram.len(), 2);
        assert_eq!(a.start, -5);
        assert_eq!(a.histogram[0], 0.5);
        assert_eq!(a.histogram[1], 1.5);
        assert_eq!(a.mean, a.mean());
        assert_eq!(a.exists(), true);
    }

    #[test]
    fn shift() {
        let a = Distribution::uniform(-5, 2).shift(3);
        assert_eq!(a.histogram.len(), 2);
        assert_eq!(a.start, -2);
        assert_eq!(a.mean, -1.5);
        assert_eq!(a.histogram[0], 0.5);
        assert_eq!(a.histogram[1], 0.5);
    }

    #[test]
    fn normalize_with_histogram() {
        let mut a = Distribution::uniform(5, 3);
        a.histogram[0] = 0.1;
        a.histogram[1] = 0.3;
        a.histogram[2] = 0.1;
        a.mean = 6.0;
        a.feasible_probability = 0.5;
        a.normalize();
        assert_eq!(a.histogram.len(), 3);
        assert_eq!(a.start, 5);
        assert_eq!(a.mean, 12.0);
        assert_eq!(a.histogram[0], 0.2);
        assert_eq!(a.histogram[1], 0.6);
        assert_eq!(a.histogram[2], 0.2);
    }

    #[test]
    fn normalize_with_feasibility_0() {
        let mut a = Distribution::uniform(5, 3);
        a.histogram[0] = 0.1;
        a.histogram[1] = 0.3;
        a.histogram[2] = 0.1;
        a.mean = 6.0;
        a.feasible_probability = 0.0;
        a.normalize();
        assert_eq!(a.histogram.len(), 3);
        assert_eq!(a.start, 5);
        assert_eq!(a.mean, 6.0);
        assert_eq!(a.histogram[0], 0.1);
        assert_eq!(a.histogram[1], 0.3);
        assert_eq!(a.histogram[2], 0.1);
    }

    #[test]
    fn normalize_with_feasibility_mean_only() {
        let mut a = Distribution::uniform(5, 0);
        a.mean = 55.0;
        a.feasible_probability = 0.5;
        a.normalize_with(true, 0.0);
        assert_eq!(a.histogram.len(), 0);
        assert_eq!(a.start, 5);
        assert_eq!(a.mean, 110.0);
    }
    
    #[test]
    fn normalize_with_epsilon() {
        let mut a = Distribution::uniform(5, 4);
        a.histogram[0] = 0.05;
        a.histogram[1] = 0.3;
        a.histogram[2] = 0.1;
        a.histogram[3] = 0.05;
        a.mean = 3.0;
        a.feasible_probability = 0.5;
        a.normalize_with(false, 0.07);
        assert_eq!(a.histogram.len(), 2);
        assert_eq!(a.start, 6);
        assert_eq!(a.mean, 6.0);
        assert_eq!(a.histogram[0], 0.75);
        assert_eq!(a.histogram[1], 0.25);
        assert_eq!(a.mean(), 6.25);
    }

    #[test]
    fn before_apart() {
        let a = Distribution::uniform(5, 2);
        let b = Distribution::uniform(8, 2);
        assert_eq!(a.before_probability(&b, 0), 1.0);
        assert_eq!(a.before_probability(&b, 1), 1.0);
        assert_eq!(a.before_probability(&b, 2), 1.0);
        assert_eq!(a.before_probability(&b, 3), 0.75);
        assert_eq!(a.before_probability(&b, 4), 0.25);
        assert_eq!(a.before_probability(&b, 5), 0.0);        
    }

    #[test]
    fn before_overlap() {
        let a = Distribution::uniform(5, 2);
        let b = Distribution::uniform(6, 2);
        assert_eq!(a.before_probability(&b, 0), 1.0);
        assert_eq!(a.before_probability(&b, 1), 0.75);
        assert_eq!(a.before_probability(&b, 2), 0.25);
        assert_eq!(a.before_probability(&b, 3), 0.0);
    }

    #[test]
    fn before_triangle_overlap() {
        let mut a = Distribution::uniform(5, 3);
        a.histogram[0] = 0.2;
        a.histogram[1] = 0.6;
        a.histogram[2] = 0.2;
        let mut b = Distribution::uniform(6, 3);
        b.histogram[0] = 0.2;
        b.histogram[1] = 0.5;
        b.histogram[2] = 0.3;
        assert_float_relative_eq!(a.before_probability(&b, -1), 1.0);
        assert_float_relative_eq!(a.before_probability(&b, 0), 0.2+0.6+0.2*(0.5+0.3));
        assert_float_relative_eq!(a.before_probability(&b, 1), 0.2+0.6*(0.5+0.3)+0.2*0.3);
        assert_float_relative_eq!(a.before_probability(&b, 2), 0.2*(0.5+0.3)+0.6*0.3);
        assert_float_relative_eq!(a.before_probability(&b, 3), 0.2*0.3);
        assert_float_relative_eq!(a.before_probability(&b, 4), 0.0);
    }

    #[test]
    fn before_apart_after() {
        let a = Distribution::uniform(8, 2);
        let b = Distribution::uniform(5, 2);
        assert_eq!(a.before_probability(&b, -4), 1.0);
        assert_eq!(a.before_probability(&b, -3), 0.75);
        assert_eq!(a.before_probability(&b, -2), 0.25);
        assert_eq!(a.before_probability(&b, -1), 0.0);
        assert_eq!(a.before_probability(&b, 0), 0.0);
        assert_eq!(a.before_probability(&b, 1), 0.0);
        assert_eq!(a.before_probability(&b, 2), 0.0);        
    }

    #[test]
    fn test_from_buckets() {
        let buckets = vec![(-1..3, 50), (5..7, 50), (7..7, 22), (0..0, 5)];
        let a = Distribution::from_buckets(buckets, 100);
        assert_eq!(a.histogram.len(), 8);
        assert_eq!(a.start, -1);
        assert_eq!(a.mean, 3.0);
        assert_float_relative_eq!(a.feasible_probability, 0.95238095238);
        assert_float_relative_eq!(a.histogram[0], 0.125);
        assert_float_relative_eq!(a.histogram[1], 0.125);
        assert_float_relative_eq!(a.histogram[2], 0.125);
        assert_float_relative_eq!(a.histogram[3], 0.125);
        assert_float_relative_eq!(a.histogram[4], 0.0);
        assert_float_relative_eq!(a.histogram[5], 0.0);
        assert_float_relative_eq!(a.histogram[6], 0.25);
        assert_float_relative_eq!(a.histogram[7], 0.25);
    }

    #[test]
    fn test_from_buckets_nonegative() {
        let buckets = vec![(-5..-3, 25), (-1..3, 50), (6..8, 25), (0..0, 5)];
        let mut a = Distribution::from_buckets(buckets, 100);
        a.nonnegative();
        assert_eq!(a.histogram.len(), 8);
        assert_eq!(a.start, 0);
        assert_eq!(a.mean, 2.0);
        assert_float_relative_eq!(a.feasible_probability, 0.95238095238);
        assert_float_relative_eq!(a.histogram[0], 0.5);
        assert_float_relative_eq!(a.histogram[1], 0.125);
        assert_float_relative_eq!(a.histogram[2], 0.125);
        assert_float_relative_eq!(a.histogram[3], 0.0);
        assert_float_relative_eq!(a.histogram[4], 0.0);
        assert_float_relative_eq!(a.histogram[5], 0.0);
        assert_float_relative_eq!(a.histogram[6], 0.125);
        assert_float_relative_eq!(a.histogram[7], 0.125);
    }
}