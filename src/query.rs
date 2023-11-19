
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
    trace: HashMap<ByAddress<&'a connection::Connection<'a>>, f32>
}

impl<'a> Query<'a> {
    fn recursive(&mut self, c: &'a connection::Connection<'a>) {
        if c.destination_arrival.borrow().exists() {
            return;
        }
        if c.to.id == self.destination.id {
            c.destination_arrival.replace(self.store.delay_distribution(&c.arrival, false, c.product_type, self.now));
            return;
        }
        if self.trace.contains_key(&ByAddress(c)) {
            return;
        }
        self.trace.insert(ByAddress(c), 1.0);
        for dep in &*c.to.departures.borrow() {
            self.recursive(dep);
        }
        self.trace.remove(&ByAddress(c));
        
        let mut departures_By_arrival = c.to.departures.borrow().clone();
        departures_By_arrival.sort_by(|a, b| a.destination_arrival.borrow().mean.partial_cmp(&b.destination_arrival.borrow().mean).unwrap());

        let mut remaining_probability = 1.0;
        let mut new_distribution = distribution::Distribution::empty(c.arrival.scheduled);
        let mut last_mean_departure = 0.;
        for dep in &departures_By_arrival {
            let mean = self.store.delay_distribution(&dep.departure, true, dep.product_type, self.now).mean;
            if mean < last_mean_departure {
                continue;
            }
            let dest = &*dep.destination_arrival.borrow();
            let mut p = self.store.reachable_probability(&c.arrival, c.product_type, &dep.departure, dep.product_type, self.now);
            p *= dest.feasible_probability;            
            new_distribution.add(dest, p*remaining_probability);
            remaining_probability *= 1.0-p;
            last_mean_departure = mean;
        }
        new_distribution.feasible_probability = 1.0-remaining_probability;
        c.destination_arrival.replace(new_distribution);
    }
}