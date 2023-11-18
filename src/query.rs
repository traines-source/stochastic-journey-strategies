
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
        now: now
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
    now: types::Mtime
}

impl Query<'_> {
    fn recursive<'a>(&mut self, c: &'a connection::Connection) {
        if c.destination_arrival.borrow().exists() {
            return;
        }
        if c.to.id == self.destination.id {
            c.destination_arrival.replace(self.store.delay_distribution(&c.arrival, false, c.product_type, self.now));
            return;
        }
        for dep in &*c.to.departures.borrow() {
            self.recursive(dep);
        }
        let mut departuresByArrival = c.to.departures.borrow().clone();
        departuresByArrival.sort_by(|a, b| a.destination_arrival.borrow().mean.partial_cmp(&b.destination_arrival.borrow().mean).unwrap());

        let mut remaining_probability = 1.0;
        let mut new_distribution = distribution::Distribution::empty(c.arrival.scheduled);
        let mut last_mean_departure = 0.;
        for dep in &departuresByArrival {
            let mean = self.store.delay_distribution(&dep.departure, true, dep.product_type, self.now).mean;
            if mean < last_mean_departure {
                continue;
            }
            let p = self.store.reachable_probability(&c.arrival, c.product_type, &dep.departure, dep.product_type, self.now);
            new_distribution.add(&*dep.destination_arrival.borrow(), p*remaining_probability);
            remaining_probability *= 1.0-p;
            last_mean_departure = mean;
        }
        c.destination_arrival.replace(new_distribution);
    }
}