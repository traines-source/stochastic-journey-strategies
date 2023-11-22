
use std::collections::HashMap;
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
        trace: HashMap::new()
    };
    for dep in &*origin.departures.borrow() {
        q.recursive(dep);
    }
}

struct Query<'a> {
    store: &'a mut distribution_store::Store,
    destination: &'a connection::Station<'a>,
    start_time: types::Mtime,
    max_time: types::Mtime,
    now: types::Mtime,
    trace: HashMap<*const connection::Connection<'a>, f32>
}

impl<'a, 'b> Query<'a> {
    fn recursive(&mut self, c: &'b connection::Connection<'a>) {
        if c.destination_arrival.borrow().exists() {
            return;
        }
        if c.to.id == self.destination.id {
            c.destination_arrival.replace(self.store.delay_distribution(&c.arrival, false, c.product_type, self.now));
            return;
        }
        if self.trace.contains_key(&(c as *const connection::Connection<'a>)) {
            return;
        }
        self.trace.insert(c as *const connection::Connection<'a>, 1.0);
        let binding = c.to.departures.borrow();
        for dep in &*binding {
            self.recursive(dep);
        }
        self.trace.remove(&(c as *const connection::Connection<'a>));
        
        
        let mut departures_by_arrival: Vec<&connection::Connection<'a>> = binding.iter().collect();
        departures_by_arrival.sort_by(|a, b| a.destination_arrival.borrow().mean.partial_cmp(&b.destination_arrival.borrow().mean).unwrap());

        let mut remaining_probability = 1.0;
        let mut new_distribution = distribution::Distribution::empty(c.arrival.scheduled);
        let mut last_mean_departure = 0.;
        for dep in &departures_by_arrival {
            let mean = self.store.delay_distribution(&dep.departure, true, dep.product_type, self.now).mean;
            if mean < last_mean_departure {
                continue;
            }
            let dest = &*dep.destination_arrival.borrow();
            let mut p = 1.0;
            if c.trip_id != dep.trip_id || ByAddress(c.route) != ByAddress(dep.route) {
                p = self.store.reachable_probability(&c.arrival, c.product_type, &dep.departure, dep.product_type, self.now);
            }
            p *= dest.feasible_probability;
            if p > 0.0 {
                new_distribution.add(dest, p*remaining_probability);
                remaining_probability *= 1.0-p;
                last_mean_departure = mean;
            }
        }
        new_distribution.feasible_probability = 1.0-remaining_probability;
        c.destination_arrival.replace(new_distribution);
    }
}