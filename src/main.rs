use std::cmp;
mod distribution;
mod types;

struct Route {
	id: String,
	name: String,
	product_type: i16,
	message: String,
	direction: String
}

struct Station<'a> {
	id: String,
	name: String,
	departures: &'a mut [Connection<'a>],
	lat: f32,
	lon: f32
}

struct Connection<'a> {
	route: &'a Route,
	from: &'a Station<'a>,
	to: &'a mut Station<'a>,
	departure: StopInfo,
	arrival: StopInfo,
	message: String,
	cancelled: bool,
	product_type: i16,
	destination_arrival: distribution::Distribution
}

struct StopInfo {
	scheduled: types::Mtime,
	delay: i16,
	scheduled_track: String,
	projected_track: String
}


fn main() {
    println!("Hello, world!");
}


fn query(origin: &Station, destination: &Station, start_time: types::Mtime, max_time: types::Mtime) {

}

fn distribution(t: &StopInfo) -> distribution::Distribution {
	distribution::Distribution{
        histogram: vec![1.1],
        start: 0,
		mean: 0.0,
    }
}

fn reachable_probability(arr: &StopInfo, dep: &StopInfo) -> f32 {
	return 1.0
}

fn recursive<'a>(c: &'a mut Connection, destination: &Station) {
	if c.to.id == destination.id {
		c.destination_arrival = distribution(&c.arrival);
		return;
	}
	if c.destination_arrival.histogram.len() > 0 {
		return;
	}
	for dep in &mut *c.to.departures {
		recursive(dep, destination);
	}
	c.to.departures.sort_by(|a, b| a.destination_arrival.mean.partial_cmp(&b.destination_arrival.mean).unwrap());
	let mut remaining_probability = 1.0;
    //let mut new_distribution;
	for dep in &mut *c.to.departures {
		let p = reachable_probability(&c.arrival, &dep.departure);
        //new_distribution = sumDistributions(newDistribution, dep.destination_arrival, p);
        remaining_probability *= 1.0-p;
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
