use std::cmp;
use crate::types;

pub struct Distribution {
	pub histogram: Vec<f32>,
	pub start: types::Mtime,
	pub mean: f32
}

impl Distribution {

	pub fn exists(&self) -> bool {
        self.histogram.len() > 0
    }

    pub fn empty(start: types::Mtime) -> Distribution {
        Distribution{
            histogram: vec![],
            start: start,
            mean: 0.
        }
    }

    pub fn uniform(start: types::Mtime, width: usize) -> Distribution {
        if width == 0 {
            return Distribution::empty(start);
        }
        Distribution{
            histogram: vec![1.0/(width as f32); width],
            start: start,
            mean:  start as f32+((width-1) as f32/2.0)
        }
    }

    fn mean(&self) -> f32 {
        let mut mean = 0.0;
        for i in 0..self.histogram.len() {
            mean += (self.start as f32+i as f32)*self.histogram[i];
        }
        mean
    }
    
    pub fn add(&mut self, other: &Distribution, weight: f32) {
        let self_start = self.start as usize;
        let other_start = other.start as usize;
        let start = cmp::min(self_start, other_start);
		let end = cmp::max(self_start+self.histogram.len(), other_start+other.histogram.len());
		let self_offset = self_start-start;
		let other_offset = other_start-start;
		let mut h = vec![0.; end-start];

		for i in 0..(end-start) {
			if i >= self_offset && i-self_offset < self.histogram.len() {
				h[i] += self.histogram[i-self_offset];
			}
			if i >= other_offset && i-other_offset < other.histogram.len() {
				h[i] += other.histogram[i-other_offset]*weight;
			}
		}
        self.histogram = h;
		self.start = start as types::Mtime;
        self.mean = self.mean + other.mean*weight;
    }

    pub fn shift(&self, start: types::Mtime) -> Distribution {
        Distribution{
            histogram: self.histogram.clone(),
            start: self.start+start,
            mean: self.mean+start as f32
        }
    }

    pub fn before_probability(&self, other: &Distribution, offset: i32) -> f32 {
        let mut p = 0.0;
        let selfLen = self.histogram.len() as i32;
        let otherLen = other.histogram.len() as i32;
        for i in 0..selfLen {
            for j in 0..otherLen {
                if self.start+i+offset > other.start+j {
                    continue
                }
                p += self.histogram[i as usize]*other.histogram[j as usize];
            }
        }
        if p > 1.0 {
            return 1.0
        }
        p
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
    #[should_panic]
    fn add_negative() {
        let mut a = Distribution::empty(-5);
        let b = Distribution::empty(0);
        a.add(&b, 1.0);
    }

    #[test]
    fn add_empty() {
        let mut a = Distribution::empty(0);
        let b = Distribution::empty(0);
        a.add(&b, 1.0);
        assert_eq!(a.histogram.len(), 0);
        assert_eq!(a.start, 0);
        assert_eq!(a.mean, a.mean());
    }

    #[test]
    fn add_empty_apart() {
        let mut a = Distribution::empty(10);
        let b = Distribution::empty(5);
        a.add(&b, 1.0);
        assert_eq!(a.histogram.len(), 5);
        assert_eq!(a.start, 5);
        assert_eq!(a.mean, a.mean());
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
    fn shift() {
        let a = Distribution::uniform(-5, 2).shift(3);
        assert_eq!(a.histogram.len(), 2);
        assert_eq!(a.start, -2);
        assert_eq!(a.histogram[0], 0.5);
        assert_eq!(a.histogram[1], 0.5);
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
}