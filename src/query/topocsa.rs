use crate::distribution;
use crate::distribution_store;
use crate::connection;
use crate::types;

use std::collections::HashMap;

use by_address::ByAddress;
use indexmap::IndexMap;
use std::collections::HashSet;

struct ConnectionLabel {
    visited: i16,
    order: i32
}

pub fn query<'a>(store: &'a mut distribution_store::Store, origin: &'a connection::Station, destination: &'a connection::Station, start_time: types::Mtime, max_time: types::Mtime, now: types::Mtime) {
    //recursive::query(store, origin, destination, start_time, max_time, now);
}

struct Query<'a> {
    store: &'a mut distribution_store::Store,
    destination: &'a connection::Station,
    start_time: types::Mtime,
    max_time: types::Mtime,
    now: types::Mtime,
}

impl<'a, 'b> Query<'a> {

    fn dfs(&mut self, anchor_id: usize, labels: &mut HashMap<usize, ConnectionLabel>, topo_idx: &mut i32, connections: &[connection::Connection], cut: &mut HashSet<(usize, usize)>) {
        let mut stack: Vec<usize> = vec![];
        let mut trace: IndexMap<usize, usize> = IndexMap::new();
        stack.push(anchor_id);
        labels.insert(anchor_id, ConnectionLabel{visited: 0, order: 0});

        while !stack.is_empty() {
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
                assert_eq!(trace.pop().unwrap().1, stack.len());
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
                                stack.pop();
                                continue;
                            }
                            let cut_before = trace.get_index(min_i).unwrap();
                            let cut_after = trace.get_index(min_i-1).unwrap();
                            cut.insert((*cut_after.0, *cut_before.0));
                            stack.truncate(*cut_before.1);
                            trace.truncate(min_i);
                            break;
                        } else {
                            panic!("marked as visited but not in trace");
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
        cut
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