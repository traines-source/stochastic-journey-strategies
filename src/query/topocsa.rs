use crate::distribution;
use crate::distribution_store;
use crate::connection;
use crate::types;

use std::collections::HashMap;

use by_address::ByAddress;
use indexmap::IndexMap;
use std::collections::HashSet;



pub fn query<'a>(store: &'a mut distribution_store::Store, connections: &mut Vec<connection::Connection<'a>>, origin: &'a connection::Station, destination: &'a connection::Station, start_time: types::Mtime, max_time: types::Mtime, now: types::Mtime) {
    let mut q = Query {
        store: store,
        destination: destination,
        start_time: start_time,
        max_time: max_time,
        now: now
    };
    q.preprocess(connections);
    q.csa(connections);
    q.store.clear_reachability();
}

struct Query<'a> {
    store: &'a mut distribution_store::Store,
    destination: &'a connection::Station,
    start_time: types::Mtime,
    max_time: types::Mtime,
    now: types::Mtime,
}

struct ConnectionLabel {
    visited: i16,
    order: i32
}

impl<'a, 'b> Query<'a> {

    fn dfs(&mut self, anchor_id: usize, labels: &mut HashMap<usize, ConnectionLabel>, topo_idx: &mut i32, connections: &[connection::Connection], cut: &mut HashSet<(usize, usize)>) {
        let mut stack: Vec<usize> = vec![];
        let mut trace: IndexMap<usize, usize> = IndexMap::new();
        stack.push(anchor_id);
        labels.insert(anchor_id, ConnectionLabel{visited: 0, order: 0});

        while !stack.is_empty() {

            println!("loop {:?} {:?}", stack, trace);
            let c_id = *stack.last().unwrap();
            let c_label = labels.get_mut(&c_id).unwrap();
            if c_label.visited == 0 {
                c_label.visited = 1;
                trace.insert(c_id, stack.len()-1);
            } else {
                if c_label.visited == 1 {
                    c_label.order = *topo_idx;
                    *topo_idx += 1;
                }
                c_label.visited = 2;
                stack.pop();
                println!("{:?} {:?}", stack, trace);
                let p = trace.pop().unwrap();
                assert_eq!(p.0, c_id);
                assert_eq!(p.1, stack.len());
                continue;
            }
            let c = connections.get(c_id).unwrap();
            let deps = c.to.departures.borrow();
            for dep_id in &*deps {
                let dep = connections.get(*dep_id).unwrap();
                let dep_label = labels.get(dep_id);
                if cut.contains(&(c_id, *dep_id)) {
                    continue;
                }
                // TODO max reachability independent from now
                let reachable = self.store.reachable_probability_conn(c, dep, self.now);
                if reachable == 0.0 {
                    continue;
                }
                if dep_label.is_some() {
                    let dep_label = dep_label.unwrap();
                    if dep_label.visited == 1 {
                        let trace_idx = trace.get_index_of(dep_id);
                        if trace_idx.is_some() {
                            let transfer_time = dep.departure.projected()-c.arrival.projected();
                            let mut min_transfer = transfer_time;
                            let mut min_i = trace.len();
                            let start = trace_idx.unwrap()+1 as usize;
                            for i in start..trace.len() {
                                let test = trace.get_index(i).unwrap();
                                let t = connections.get(*test.0).unwrap().departure.projected()-connections.get(*trace.get_index(i-1).unwrap().0).unwrap().arrival.projected();
                                if t < min_transfer {
                                    min_transfer = t;
                                    min_i = i;
                                }
                            }
                            if min_transfer > 0 {
                                panic!("cutting positive transfer {:?} {:?} {} {}", c.departure, c.route, min_transfer, transfer_time)
                            }
                            if min_i == trace.len() {
                                cut.insert((c_id, *dep_id));
                                continue;
                            }
                            let cut_before = trace.get_index(min_i).unwrap();
                            let cut_after = trace.get_index(min_i-1).unwrap();
                            cut.insert((*cut_after.0, *cut_before.0));
                            stack.truncate(*cut_before.1);
                            println!("{:?}", trace);
                            for _ in min_i..trace.len() {
                                let l = labels.get_mut(&trace.pop().unwrap().0).unwrap();
                                assert_eq!(l.visited, 1);
                                l.visited = 0;
                            }
                            break;
                        } else {
                            panic!("marked as visited but not in trace {:?} {:?}", *dep_id, trace);
                        }
                    } else if dep_label.visited == 2 {
                        continue;
                    }
                }
                stack.push(*dep_id);
                labels.insert(*dep_id, ConnectionLabel { visited: 0, order: 0 });
            }
        }
    }
    
    pub fn preprocess(&mut self, connections: &mut Vec<connection::Connection>) -> HashSet<(usize, usize)> {
        println!("Start preprocessing...");
        let mut labels: HashMap<usize, ConnectionLabel> = HashMap::with_capacity(connections.len());
        let mut cut: HashSet<(usize, usize)> = HashSet::new();
        let mut topo_idx = 0;
        
        for i in 0..connections.len() {
            if !labels.contains_key(&i) || labels.get(&i).unwrap().visited != 2 {
                self.dfs(i, &mut labels, &mut topo_idx, connections, &mut cut);
                println!("connections {} cycles found {} labels {} done {}", connections.len(), cut.len(), labels.len(), i);
            }
        }
        println!("Done DFSing.");
        connections.sort_by(|a, b|
            labels.get(&a.id).unwrap().order.partial_cmp(&labels.get(&b.id).unwrap().order).unwrap()
        );
        println!("Done preprocessing.");
        println!("{:?}", cut);
        cut
    }

    fn csa(&mut self, connections: &[connection::Connection]) {
        let mut station_labels: HashMap<&str, Vec<usize>> = HashMap::new();
        for i in 0..connections.len() {
            let c = connections.get(i).unwrap();
            if c.cancelled {
                c.destination_arrival.replace(Some(distribution::Distribution::empty(c.arrival.scheduled)));
                continue;
            }
            if !station_labels.contains_key(&c.to.id as &str) {
                station_labels.insert(&c.to.id, vec![]);
            }
            if !station_labels.contains_key(&c.from.id as &str) {
                station_labels.insert(&c.from.id, vec![]);
            }
            let mut new_distribution = distribution::Distribution::empty(c.arrival.scheduled);
            if c.to.id == self.destination.id {
                new_distribution = self.store.delay_distribution(&c.arrival, false, c.product_type, self.now);
            } else {
                let mut remaining_probability = 1.0;
                let mut last_departure: Option<distribution::Distribution> = None;
                let departures = station_labels.get(&c.to.id as &str).unwrap();
                for dep_id in  departures.iter().rev() {
                    let dep = connections.get(*dep_id).unwrap();
                    let dest = dep.destination_arrival.borrow();
                    let mut p: f32 = dest.as_ref().map(|da| da.feasible_probability).unwrap_or(0.0);
                    if p < 0.001 {
                        continue;
                    }
                    if expect_float_absolute_eq!(dest.as_ref().unwrap().mean, 0.0, 1e-3).is_ok() {
                        panic!("mean 0 with high feasibility");
                    }
                    assert_float_absolute_eq!(dest.as_ref().unwrap().mean, dest.as_ref().unwrap().mean(), 1e-3);

                    let dep_dist = self.store.delay_distribution(&dep.departure, true, dep.product_type, self.now);
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
                        if remaining_probability < 0.001 {
                            break;
                        }
                    }
                }
                new_distribution.feasible_probability = (1.0-remaining_probability).clamp(0.0,1.0);
                if new_distribution.feasible_probability < 1.0 {
                    new_distribution.normalize();
                }
            }
            let station_label = station_labels.get_mut(&c.from.id as &str);
            let departures = station_label.unwrap();
            let mut found = false;
            for j in (0..departures.len()).rev() {
                let dom = connections.get(departures[j]).unwrap().destination_arrival.borrow();
                let dom_dest_dist = dom.as_ref().unwrap();
                if new_distribution.mean < dom_dest_dist.mean {
                    departures.insert(j+1, i);
                    found = true;
                    break;
                } 
            }
            if !found {
                departures.insert(0, i);
            }
            c.destination_arrival.replace(Some(new_distribution));
        }
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_compiles() {
        let mut store = distribution_store::Store::new();
        let s = connection::Station::new("id".to_string(), "name".to_string(), vec![]);
        let mut q = Query {
            store: &mut store,
            destination: &s,
            start_time: 0,
            max_time: 0,
            now: 0
        };
        let mut connections: Vec<connection::Connection> = vec![];
        let cut = q.preprocess(&mut connections);
        assert_eq!(cut.len(), 0);
    }
}