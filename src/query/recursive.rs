
use std::collections::HashMap;
use std::collections::HashSet;

use indexmap::IndexMap;
use by_address::ByAddress;

use crate::distribution;
use crate::distribution_store;
use crate::connection;
use crate::types;

pub fn query<'a, 'b>(store: &'b mut distribution_store::Store, connections: &mut Vec<connection::Connection<'a>>, origin: &'a connection::Station, destination: &'a connection::Station, start_time: types::Mtime, max_time: types::Mtime, now: types::Mtime, cut: HashSet<(usize, usize)> ) {
    let mut q = Query {
        store: store,
        destination: destination,
        start_time: start_time,
        max_time: max_time,
        now: now,
        trace: IndexMap::new(),
        visited: HashMap::new(),
        cycles_found: 0,
        cycles_cut: 0,
        cycles_cut_direct: 0,
        connections: 0,
        cut: HashSet::new()
    };
    //for dep in &*origin.departures.borrow() {
    for dep in connections.iter() {
        q.recursive(dep.id, connections, 1.0);
    }
    println!("cut: {}", q.cut.len());
    if cut.len() > 0 {
        assert_eq!(q.cut.len(), cut.len());
    }
    q.store.clear_reachability();
}

struct Query<'a> {
    store: &'a mut distribution_store::Store,
    destination: &'a connection::Station,
    start_time: types::Mtime,
    max_time: types::Mtime,
    now: types::Mtime,
    trace: IndexMap<usize, f32>,
    visited: HashMap<&'a str, usize>,
    cycles_found: i32,
    cycles_cut: i32,
    cycles_cut_direct: i32,
    connections: i32,
    cut: HashSet<(usize, usize)>,
}

impl<'a, 'b> Query<'a> {
    fn recursive(&mut self, c_id: usize, connections: &[connection::Connection<'a>], reachable_p: f32) -> Option<usize> {
        let c = connections.get(c_id).unwrap();
        if c.destination_arrival.borrow().is_some() {
            return None;
        }
        
        self.trace.insert(c_id, reachable_p);
        let binding = c.to.departures.borrow();
        for dep_id in &*binding {
            let dep = connections.get(*dep_id).unwrap();
            if self.cut.contains(&(c_id, *dep_id)) {
                continue;
            }
            if dep.destination_arrival.borrow().is_some() {
                continue;
            }
            let p = self.store.reachable_probability_conn(c, dep, self.now);
            if p <= 0.0 {
                continue;
            }
            let idx = self.trace.get_index_of(dep_id);
            if idx.is_some() {
                self.cycles_found += 1;

                let transfer_time = dep.departure.projected()-c.arrival.projected();
                let mut min_reachability = transfer_time;
                let mut min_i = self.trace.len();
                let start = idx.unwrap()+1 as usize;
                for i in start..self.trace.len() {
                    let test = self.trace.get_index(i).unwrap();
                    let t = connections.get(*test.0).unwrap().departure.projected()-connections.get(*self.trace.get_index(i-1).unwrap().0).unwrap().arrival.projected();
                    if t < min_reachability {
                        min_reachability = t;
                        min_i = i;
                    }
                }
                if min_reachability > 0 {
                    println!("cutting high reachability {:?} {:?} {} {}", dep.departure, dep.route, min_reachability, reachable_p)
                }
                /*let mut min_reachability = reachable_p;
                let mut min_i = self.trace.len();
                let start = idx.unwrap()+1 as usize;
                for i in start..self.trace.len() {
                    let test = self.trace.get_index(i).unwrap();
                    if *test.1 < min_reachability {
                        min_reachability = *test.1;
                        min_i = i;
                    }
                }
                if min_reachability > 0.2 {
                    println!("cutting high reachability {:?} {:?} {} {}", c.departure, c.route, min_reachability, reachable_p)
                }*/
                if min_i == self.trace.len() {
                    self.cycles_cut += 1;
                    self.cycles_cut_direct += 1;
                    self.cut.insert((c_id, *dep_id));
                    continue;
                }
                //return None;
                let min_id = *self.trace.get_index(min_i).unwrap().0;
                self.cut.insert((*self.trace.get_index(min_i-1).unwrap().0, min_id));

                assert_eq!(self.trace.pop().unwrap().0, c_id);
                return Some(min_id);
            }
        }
        for dep_id in binding.iter().rev() {
            let dep = connections.get(*dep_id).unwrap();
            if dep.destination_arrival.borrow().is_none() && !self.cut.contains(&(c_id, *dep_id)) {
                let p = self.store.reachable_probability_conn(c, dep, self.now);
                if p <= 0.0 {
                    continue;
                }
                let cycle_found = self.recursive(*dep_id, connections, p);
                if cycle_found.is_some() {
                    if *dep_id == cycle_found.unwrap() {
                        self.cycles_cut += 1;
                        continue;
                    }
                    assert_eq!(self.trace.pop().unwrap().0, c_id);
                    return cycle_found;
                }
            }
        }
        assert_eq!(self.trace.pop().unwrap().0, c_id);
        if !self.visited.contains_key(&c.to.id as &str) {
            println!("finished iterating {} {} len: {} cycles: {} cut: {} direct: {} conns: {} reachs: {}", c.to.name, c.to.id, self.visited.len(), self.cycles_found, self.cut.len(), self.cycles_cut_direct, self.connections, self.store.reachability_len());
            self.visited.insert(&c.to.id as &str, self.visited.len());
        }
        if c.cancelled {
            c.destination_arrival.replace(Some(distribution::Distribution::empty(c.arrival.scheduled)));
            self.connections += 1;
            //return None;
        } else if c.to.id == self.destination.id {
            c.destination_arrival.replace(Some(self.store.delay_distribution(&c.arrival, false, c.product_type, self.now)));
            self.connections += 1;
            //return None;
        }
        if c.destination_arrival.borrow().is_some() {
            return None;
        }
        
        let mut departures_by_arrival: Vec<&usize> = binding.iter().collect();
        departures_by_arrival.sort_by(|a, b| connections.get(**a).unwrap().destination_arrival.borrow().as_ref().map(|da| da.mean).unwrap_or(0.0).partial_cmp(
            &connections.get(**b).unwrap().destination_arrival.borrow().as_ref().map(|da| da.mean).unwrap_or(0.0)).unwrap());

        let mut remaining_probability = 1.0;
        let mut new_distribution = distribution::Distribution::empty(c.arrival.scheduled);
        let mut last_departure: Option<distribution::Distribution> = None;
        for dep_id in &departures_by_arrival {
            let dep = connections.get(**dep_id).unwrap();
            let dest = dep.destination_arrival.borrow();
            if self.cut.contains(&(c.id, dep.id)) {
                continue;
            }
            let mut p = dest.as_ref().map(|da| da.feasible_probability).unwrap_or(0.0);   
            if p <= 0.0 {
                continue;
            }
            if expect_float_absolute_eq!(dest.as_ref().unwrap().mean, 0.0, 1e-3).is_ok() {
                panic!("mean 0 with high feasibility");
            }
            assert_float_absolute_eq!(dest.as_ref().unwrap().mean, dest.as_ref().unwrap().mean(), 1e-3);

            let dep_dist = self.store.delay_distribution(&dep.departure, true, dep.product_type, self.now);
            /*if last_departure.is_some() && dep_dist.mean < last_departure.as_ref().unwrap().mean {
                continue;
            }*/
            if last_departure.is_some() {
                p *= last_departure.as_ref().unwrap().before_probability(&dep_dist, 1);
            }  
            if p > 0.0 && (c.trip_id != dep.trip_id || ByAddress(c.route) != ByAddress(dep.route)) {
                p *= self.store.reachable_probability_conn(c, dep, self.now);
            }
            if p > 0.0 {
                new_distribution.add(dest.as_ref().unwrap(), (p*remaining_probability).clamp(0.0,1.0));
                remaining_probability = ((1.0-p).clamp(0.0,1.0)*remaining_probability).clamp(0.0,1.0);
                last_departure = Some(dep_dist);
                if remaining_probability <= 0.0 {
                    break;
                }
            }
        }
        new_distribution.feasible_probability = (1.0-remaining_probability).clamp(0.0,1.0);
        if new_distribution.feasible_probability < 1.0 {
            new_distribution.normalize();
        }
        c.destination_arrival.replace(Some(new_distribution));
        self.connections += 1;
        None
    }
}