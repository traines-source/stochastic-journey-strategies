use std::cmp;
mod distribution;
mod distribution_store;
mod connection;
mod types;

fn main() {
    println!("Hello, world!");
}


fn query(origin: &connection::Station, destination: &connection::Station, start_time: types::Mtime, max_time: types::Mtime) {

}

fn recursive<'a>(c: &'a mut connection::Connection, destination: &connection::Station, now: types::Mtime, store: &distribution_store::Store) {
	if c.to.id == destination.id {
		c.destination_arrival = store.delay_distribution(&c.arrival, false, c.product_type, now);
		return;
	}
	if c.destination_arrival.histogram.len() > 0 {
		return;
	}
	for dep in &mut *c.to.departures {
		recursive(dep, destination, now, store);
	}
	c.to.departures.sort_by(|a, b| a.destination_arrival.mean.partial_cmp(&b.destination_arrival.mean).unwrap());
	let mut remaining_probability = 1.0;
    let mut new_distribution = distribution::Distribution::empty(c.departure.scheduled);
	let mut last_mean_departure = 0.;
	for dep in &mut *c.to.departures {
		let mean = store.delay_distribution(&dep.departure, true, dep.product_type, now).mean;
		if mean < last_mean_departure {
			continue;
		}
		let p = store.reachable_probability(&c.arrival, c.product_type, &dep.departure, dep.product_type, now);
        new_distribution.add(&dep.destination_arrival, p);
        remaining_probability *= 1.0-p;
		last_mean_departure = mean
	}
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        assert_eq!(4, 4);
    }
}
