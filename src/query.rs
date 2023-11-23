
use indexmap::IndexMap;
use by_address::ByAddress;

use crate::distribution;
use crate::distribution_store;
use crate::connection;
use crate::types;

pub fn query<'a>(store: &'a mut distribution_store::Store, origin: &'a connection::Station<'a>, destination: &'a connection::Station<'a>, start_time: types::Mtime, max_time: types::Mtime, now: types::Mtime) {
    let mut q = Query {
        store: store,
        destination: destination,
        start_time: start_time,
        max_time: max_time,
        now: now,
        trace: IndexMap::new()
    };
    for dep in &*origin.departures.borrow() {
        q.recursive(dep, 1.0);
    }
}

struct Query<'a> {
    store: &'a mut distribution_store::Store,
    destination: &'a connection::Station<'a>,
    start_time: types::Mtime,
    max_time: types::Mtime,
    now: types::Mtime,
    trace: IndexMap<*const connection::Connection<'a>, f32>
}

impl<'a, 'b> Query<'a> {
    fn recursive(&mut self, c: &'b connection::Connection<'a>, reachable_p: f32) -> Option<*const connection::Connection<'a>> {
        if c.destination_arrival.borrow().is_some() {
            return None;
        }
        if c.to.id == self.destination.id {
            c.destination_arrival.replace(Some(self.store.delay_distribution(&c.arrival, false, c.product_type, self.now)));
            return None;
        }
        let my_address = c as *const connection::Connection<'a>;
        let idx = self.trace.get_index_of(&my_address);
        if idx.is_some() {
            let mut min_reachability = reachable_p;
            let mut min_ptr = my_address;
            let start = idx.unwrap()+1 as usize;
            for i in start..self.trace.len() {
                let test = self.trace.get_index(i).unwrap();
                if *test.1 < min_reachability {
                    min_reachability = *test.1;
                    min_ptr = *test.0;
                }
            }
            if min_ptr == my_address {
                return None;
            }
            //return None;
            return Some(min_ptr);
        }
        self.trace.insert(my_address, reachable_p);
        let binding = c.to.departures.borrow();
        for dep in &*binding {
            let p = self.store.reachable_probability(&c.arrival, c.product_type, &dep.departure, dep.product_type, self.now);
            let cycle_found = self.recursive(dep, p);
            if cycle_found.is_some() {
                assert_eq!(self.trace.pop().unwrap().0, my_address);
                if my_address == cycle_found.unwrap() {
                    return None;
                }
                return cycle_found;
            }
        }
        assert_eq!(self.trace.pop().unwrap().0, my_address);
        
        let mut departures_by_arrival: Vec<&connection::Connection<'a>> = binding.iter().collect();
        departures_by_arrival.sort_by(|a, b| a.destination_arrival.borrow().as_ref().map(|da| da.mean).unwrap_or(0.0).partial_cmp(
            &b.destination_arrival.borrow().as_ref().map(|da| da.mean).unwrap_or(0.0)).unwrap());

        let mut remaining_probability = 1.0;
        let mut new_distribution = distribution::Distribution::empty(c.arrival.scheduled);
        let mut last_mean_departure = 0.;
        for dep in &departures_by_arrival {
            let mean = self.store.delay_distribution(&dep.departure, true, dep.product_type, self.now).mean;
            if mean < last_mean_departure {
                continue;
            }
            let dest = dep.destination_arrival.borrow();
            let mut p = dest.as_ref().map(|da| da.feasible_probability).unwrap_or(0.0);
            if p > 0.0 && (c.trip_id != dep.trip_id || ByAddress(c.route) != ByAddress(dep.route)) {
                p *= self.store.reachable_probability(&c.arrival, c.product_type, &dep.departure, dep.product_type, self.now);
            }
            if p > 0.0 {
                new_distribution.add(dest.as_ref().unwrap(), (p*remaining_probability).clamp(0.0,1.0));
                remaining_probability = ((1.0-p).clamp(0.0,1.0)*remaining_probability).clamp(0.0,1.0);
                last_mean_departure = mean;
            }
        }
        new_distribution.feasible_probability = (1.0-remaining_probability).clamp(0.0,1.0);
        if new_distribution.feasible_probability < 1.0 {
            new_distribution.normalize();
        }
        c.destination_arrival.replace(Some(new_distribution));
        None
    }
}