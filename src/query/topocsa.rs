use std::cell::RefCell;
use std::collections::BTreeMap;
use std::collections::HashMap;
use std::time::Instant;
use rustc_hash::FxHashSet;
use noisy_float::types::{n32, N32};



use indexmap::IndexSet;
use serde::Deserialize;
use serde::Serialize;

use crate::distribution;
use crate::distribution_store;
use crate::connection;
use crate::types;

pub fn new<'a, 'b>(store: &'b mut distribution_store::Store, connections: &'b mut Vec<connection::Connection>, stations: &'b [connection::Station], cut: FxHashSet<(usize, usize)>, order: &'b mut Vec<usize>, now: types::Mtime, epsilon: f32, mean_only: bool) -> Environment<'b> {
    Environment {
        store: RefCell::new(store),
        connections: connections,
        stations: stations,
        now: now,
        epsilon: epsilon,
        mean_only: mean_only,
        cut: cut,
        order: order
    }
}

pub fn prepare<'a, 'b>(store: &'b mut distribution_store::Store, connections: &'b mut Vec<connection::Connection>, stations: &'b [connection::Station], order: &'b mut Vec<usize>, now: types::Mtime, epsilon: f32, mean_only: bool) -> Environment<'b> {
    let mut e = new(store, connections, stations, FxHashSet::default(), order, now, epsilon, mean_only);    
    println!("Starting topocsa...");
    e.preprocess();
    e
}

pub fn prepare_and_query<'a, 'b>(store: &'b mut distribution_store::Store, connections: &'b mut Vec<connection::Connection>, stations: &'b [connection::Station], origin: &'a connection::Station, destination: &'a connection::Station, _start_time: types::Mtime, _max_time: types::Mtime, now: types::Mtime, epsilon: f32, mean_only: bool) -> FxHashSet<(usize, usize)>  {
    let mut order = Vec::with_capacity(connections.len());
    let mut e = prepare(store, connections, stations, &mut order, now, epsilon, mean_only);
    e.query(origin, destination);
    e.store.borrow_mut().clear_reachability();
    println!("Done.");
    e.cut
}

#[derive(Debug)]
pub struct Environment<'b> {
    store: RefCell<&'b mut distribution_store::Store>,
    connections: &'b mut Vec<connection::Connection>,
    stations: &'b [connection::Station],
    now: types::Mtime,
    epsilon: f32,
    mean_only: bool,
    pub cut: FxHashSet<(usize, usize)>,
    order: &'b mut Vec<usize>
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DfsConnectionLabel {
    footpath_i: usize,
    i: usize,
    order: usize
}

pub struct ConnectionLabel {
    pub connection_idx: usize,
    pub destination_arrival: distribution::Distribution
}

#[derive(Debug)]
pub struct Instrumentation {
    max_stack: usize,
    max_trace: usize,
    unraveling_time: u128,
    before_prob_time: u128,
    unraveling_no: usize,
    cycle_sum_len: usize,
    cycle_max_len: usize,
    cycle_self_count: usize,
    encounter_2: usize,
    iterations: usize,
}

#[derive(Debug)]
struct CsaInstrumentation {
    looked_at_count: usize,
    selected_count: usize,
    new_dist_time: u128
}

impl<'a, 'b> Environment<'b> {

    fn dfs(&mut self, anchor_id: usize, topo_idx: &mut usize, labels: &mut Vec<DfsConnectionLabel>, visited: &mut Vec<i16>, stops_completed_up: &mut Vec<usize>, instr: &mut Instrumentation) {
        let mut stack: IndexSet<usize> = IndexSet::with_capacity(1000);
        stack.insert(anchor_id);
        while !stack.is_empty() {
            instr.iterations += 1;
            let c_id = *stack.last().unwrap();
            let c = &self.connections[c_id];
            let c_label = labels.get_mut(c_id).unwrap();
            let footpaths = &self.stations[c.to_idx].footpaths;
            let mut stop_idx = if c_label.footpath_i == footpaths.len() { c.to_idx } else { footpaths[c_label.footpath_i].target_location_idx };
            let mut deps = &self.stations[stop_idx].departures;
            let mut streak = false;
            if c_label.i >= stops_completed_up[stop_idx] {
                c_label.i = stops_completed_up[stop_idx];
                streak = true;
            }
            visited[c_id] = 1;
            let mut found = false;
            loop {
                if c_label.i > 0 {
                    c_label.i -= 1;
                } else if c_label.footpath_i > 0 {
                    if streak {
                        stops_completed_up[stop_idx] = 0;
                        streak = false;
                    }
                    c_label.footpath_i -= 1;
                    stop_idx = footpaths[c_label.footpath_i].target_location_idx;
                    deps = &self.stations[stop_idx].departures;
                    c_label.i = deps.len();
                    if c_label.i >= stops_completed_up[stop_idx] {
                        c_label.i = stops_completed_up[stop_idx];
                        streak = true;
                    }
                    if c_label.i == 0 {
                        streak = false;
                        continue;
                    }
                    c_label.i -= 1;
                } else {
                    break;
                }
                let dep_id = deps[c_label.i];
                let dep_visited = visited[dep_id];
                if dep_visited == 2 {
                    instr.encounter_2 += 1;
                } else {
                    if streak {
                        stops_completed_up[stop_idx] = c_label.i+1;
                        streak = false;
                    }
                    found = true;
                    let dep = &self.connections[dep_id];
                    let is_continuing = if c_label.footpath_i == footpaths.len() { c.is_consecutive(dep) } else { false };
                    if !is_continuing {
                        let transfer_time = if c_label.footpath_i == footpaths.len() { 1 } else { footpaths[c_label.footpath_i].duration as i32 };
                        let reachable = self.store.borrow_mut().before_probability(&c.arrival, c.product_type, false, &dep.departure, dep.product_type, transfer_time, self.now);
                        if reachable <= self.epsilon {
                            continue;
                        }
                    }
                    
                    if dep_visited == 1 {
                        let trace_idx = stack.get_index_of(&dep_id);
                        if trace_idx.is_some() {
                            let c = &self.connections[c_id];
                            let transfer_time = dep.departure.projected()-c.arrival.projected();
                            let mut min_transfer = if c.is_consecutive(dep) { 1 } else { transfer_time };
                            let mut min_i = stack.len();
                            let start = trace_idx.unwrap()+1 as usize;
                            instr.cycle_sum_len += stack.len()-start;
                            if stack.len()-start > instr.cycle_max_len {
                                instr.cycle_max_len = stack.len()-start;
                            }
                            for i in start..stack.len() {
                                let a = &self.connections[*stack.get_index(i-1).unwrap()];
                                let b = &self.connections[*stack.get_index(i).unwrap()];
                                if a.is_consecutive(b) {
                                    continue;
                                }
                                let t = b.departure.projected()-a.arrival.projected();
                                if t < min_transfer {
                                    min_transfer = t;
                                    min_i = i;
                                }
                            }
                            if min_i == stack.len() {
                                self.cut.insert((c_id, dep_id));
                                if c_id == dep_id {
                                    instr.cycle_self_count += 1;
                                }
                                continue;
                            }
                            let cut_before = stack.get_index(min_i).unwrap();
                            let cut_after = stack.get_index(min_i-1).unwrap();
                            self.cut.insert((*cut_after, *cut_before));
                            instr.unraveling_no += stack.len()-min_i;
                            for _ in min_i..stack.len() {
                                let id = stack.pop().unwrap();
                                let label = labels.get_mut(id).unwrap();
                                label.i += 1;
                                visited[id] = 0;
                            }
                            break;
                        } else {
                            panic!("marked as visited but not in trace {:?} {:?}", dep_id, stack);
                        }
                    } else if dep_visited != 0 {
                        panic!("unexpected visited state");
                    }
                    stack.insert(dep_id);
                    break;
                }
            }
            if !found {
                let c_label = labels.get_mut(c_id).unwrap();
                assert_eq!(c_label.i, 0);
                if streak {
                    stops_completed_up[stop_idx] = 0;
                }
                c_label.order = *topo_idx;
                *topo_idx += 1;
                visited[c_id] = 2;
                let p = stack.pop().unwrap();
                assert_eq!(p, c_id);
            }
            if stack.len() > instr.max_trace {
                instr.max_trace = stack.len();
            }
        }
        //println!("instr: {:?}", instr);
    }
    
    pub fn preprocess(&mut self) {
        println!("Start preprocessing...");
        let mut conn_ids: Vec<usize> = (0..self.connections.len()).collect();
        conn_ids.sort_unstable_by(|a,b| self.connections[*a].departure.projected().cmp(&self.connections[*b].departure.projected()));

        let mut topo_idx = 0;
        
        let mut instr = Instrumentation { max_stack: 0, max_trace: 0, unraveling_time: 0, before_prob_time: 0, unraveling_no: 0, cycle_sum_len: 0, cycle_max_len: 0, cycle_self_count: 0, encounter_2: 0, iterations: 0 };
        let start = Instant::now();
        let mut labels: Vec<DfsConnectionLabel> = Vec::with_capacity(self.connections.len());
        for c in &*self.connections {
            let footpaths = &self.stations[c.to_idx].footpaths;
            labels.push(DfsConnectionLabel {
                footpath_i: footpaths.len(),
                i: self.stations[c.to_idx].departures.len(),
                order: 0
            });
        }
        let mut stops_completed_up: Vec<usize> = Vec::with_capacity(self.stations.len());
        for s in &*self.stations {
            stops_completed_up.push(s.departures.len());
        }
        let mut visited = vec![0; self.connections.len()];
        println!("Start dfs... {}", start.elapsed().as_millis());
        self.store.borrow().print_stats();
        for idx in 0..self.connections.len() {
            let id = conn_ids[idx];
            if visited[id] != 2 {
                self.dfs(id, &mut topo_idx, &mut labels, &mut visited, &mut stops_completed_up, &mut instr);
                //println!("connections {} cycles found {} labels {} done {} {}", self.connections.len(), self.cut.len(), self.order.len(), idx, id);
            }
        }
        println!("instr: {:?}", instr);
        self.store.borrow().print_stats();
        println!("Done DFSing. {}", start.elapsed().as_millis());
        self.order.extend(labels.iter().map(|l|l.order));
        self.connections.sort_unstable_by(|a, b|
            labels[a.id].order.partial_cmp(&labels[b.id].order).unwrap()
        );
        println!("Done preprocessing.");
        println!("connections: {} topoidx: {} cut: {}", self.connections.len(), topo_idx, self.cut.len());
    }

    pub fn update(&mut self, connection_id: usize, is_departure: bool, delay: i16, cancelled: bool) {
        let c = &mut self.connections[self.order[connection_id]];
        if cancelled {
            c.cancelled = true;            
        } else if is_departure {
            c.departure.delay = Some(delay);
        } else {
            c.arrival.delay = Some(delay);
        }
    }
    pub fn query(&mut self, _origin: &'a connection::Station, destination: &'a connection::Station) -> Vec<BTreeMap<(N32, usize), ConnectionLabel>> {
        let pairs = HashMap::new();
        self.pair_query(_origin, destination, &pairs)
    }

    pub fn pair_query(&mut self, _origin: &'a connection::Station, destination: &'a connection::Station, connection_pairs: &HashMap<usize, usize>) -> Vec<BTreeMap<(N32, usize), ConnectionLabel>> {
        let mut instr = CsaInstrumentation {
            looked_at_count: 0,
            selected_count: 0,
            new_dist_time: 0
        };
        let mut station_labels: Vec<BTreeMap<(N32, usize), ConnectionLabel>> = (0..self.stations.len()).map(|i| BTreeMap::new()).collect();
        let empty_vec = vec![];
        for i in 0..self.connections.len() {
            if connection_pairs.len() > 0 && !connection_pairs.contains_key(&i) {
                continue;
            }
            let c = &self.connections[i];
            if c.cancelled {
                c.destination_arrival.replace(Some(distribution::Distribution::empty(c.arrival.scheduled))); //TODO remove
                continue;
            }            
            let mut new_distribution = distribution::Distribution::empty(c.arrival.scheduled);
            if self.stations[c.to_idx].id == destination.id {
                // TODO cancelled prob twice??
                new_distribution = self.store.borrow().delay_distribution(&c.arrival, false, c.product_type, self.now);
            } else {
                let mut footpath_distributions = vec![];
                let footpaths = &self.stations[c.to_idx].footpaths;
                for f in footpaths {
                    let mut footpath_dest_arr = distribution::Distribution::empty(0);
                    if self.stations[f.target_location_idx].id == destination.id {
                        // TODO cancelled prob twice??
                        footpath_dest_arr = self.store.borrow().delay_distribution(&c.arrival, false, c.product_type, self.now).shift(f.duration as i32);
                    } else {
                        self.new_destination_arrival(f.target_location_idx, 0, -1, 0, c.product_type, &c.arrival, f.duration as i32, &station_labels, &empty_vec, &mut footpath_dest_arr, &mut instr);   
                    }
                    if footpath_dest_arr.feasible_probability > 0.0 {
                        footpath_distributions.push(footpath_dest_arr);
                    }
                }
                //println!("{:?} {:?}", footpath_distributions.len(), footpaths.len());
                footpath_distributions.sort_unstable_by(|a, b| a.mean.partial_cmp(&b.mean).unwrap());
                self.new_destination_arrival(c.to_idx, c.id, c.trip_id, c.route_idx, c.product_type, &c.arrival, 1, &station_labels, &footpath_distributions, &mut new_distribution, &mut instr);   
            }
            let departure_conn_idx = if connection_pairs.len() == 0 { i } else { connection_pairs[&i] };
            let departure_conn = if connection_pairs.len() == 0 { c } else { &self.connections[connection_pairs[&i]] };
            let departure_station_idx = departure_conn.from_idx;
            let departures = station_labels.get_mut(departure_station_idx).unwrap();
            departure_conn.destination_arrival.replace(Some(new_distribution.clone())); // TODO remove
            if new_distribution.feasible_probability > 0.0 {
                departures.insert((n32(new_distribution.mean), c.id), ConnectionLabel{connection_idx: departure_conn_idx, destination_arrival: new_distribution});
                /*let mut j = departures.len() as i32-1;
                while j >= 0 {
                    let dom_dest_dist = &departures[j as usize].destination_arrival;
                    if new_distribution.mean < dom_dest_dist.mean {
                        break;
                    }
                    j -= 1;
                }
                let mut do_insert = true;
                if ((j+1) as usize) < departures.len() {
                    let reference = &self.connections[departures[(j+1) as usize].connection_idx];
                    let dep_dist_ref = self.store.borrow_mut().delay_distribution(&reference.departure, true, reference.product_type, self.now).mean; // TODO opt
                    let dep_dist_c = self.store.borrow_mut().delay_distribution(&departure_conn.departure, true, departure_conn.product_type, self.now).mean; // TODO opt
                    if dep_dist_c < dep_dist_ref {
                        //do_insert = false;
                    }
                }
                if do_insert {
                    departures.insert((j+1) as usize, ConnectionLabel{connection_idx: departure_conn_idx, destination_arrival: new_distribution});
                }*/
            }
        }
        println!("instr {:?}", instr);
        station_labels
    }

    fn new_destination_arrival<'c>(&'c self, station_idx: usize, c_id: usize, from_trip_id: i32, from_route_idx: usize, from_product_type: i16, from_arrival: &connection::StopInfo, transfer_time: i32, station_labels: &[BTreeMap<(N32, usize), ConnectionLabel>], footpath_distributions: &[distribution::Distribution], new_distribution: &mut distribution::Distribution, instr: &mut CsaInstrumentation) {
        let start_ts = Instant::now();
        let mut remaining_probability = 1.0;
        let mut last_departure: Option<&connection::StopInfo> = None;
        let mut last_product_type: i16 = 0;
        let departures = station_labels.get(station_idx).unwrap();

        let mut departures_i = departures.iter().peekable();
        let mut footpaths_i = 0;
        while departures_i.peek().is_some() || footpaths_i < footpath_distributions.len() {
            let mut dest_arr_dist = None;
            let mut departure = None;
            let mut departure_product_type = 0;
            let mut is_continuing = false;
            if footpaths_i < footpath_distributions.len() {
                let c = self.connections.get(c_id).unwrap();
                dest_arr_dist = Some(&footpath_distributions[footpaths_i]);
                departure = Some(&c.arrival);
                departure_product_type = c.product_type;
                is_continuing = true;
            }
            let mut dest = None; // TODO ugly
            if departures_i.peek().is_some() {
                let label = departures_i.peek().unwrap().1;
                let dep = &self.connections[label.connection_idx];
                if self.cut.contains(&(c_id, dep.id)) {
                    departures_i.next();
                    continue;
                }
                dest = Some(&label.destination_arrival);
                let candidate = dest.unwrap();
                if dest_arr_dist.is_some_and(|d| candidate.mean > d.mean) {
                    footpaths_i += 1;
                } else {
                    departures_i.next();
                    dest_arr_dist = Some(candidate);
                    departure = Some(&dep.departure);
                    departure_product_type = dep.product_type;
                    is_continuing = from_trip_id == dep.trip_id && from_route_idx == dep.route_idx && from_arrival.scheduled <= dep.departure.scheduled
                }
            } else {
                footpaths_i += 1;
            }
            instr.looked_at_count += 1;
            let mut p: f32 = dest_arr_dist.unwrap().feasible_probability;
            if p <= self.epsilon {
                continue;
            }
            if expect_float_absolute_eq!(dest_arr_dist.unwrap().mean, 0.0, 1e-3).is_ok() {
                panic!("mean 0 with high feasibility");
            }
            //assert_float_absolute_eq!(dest.as_ref().unwrap().mean, dest.as_ref().unwrap().mean(), 1e-3);
            if last_departure.is_some() {
                p *= self.store.borrow_mut().before_probability(last_departure.unwrap(), last_product_type, true, departure.unwrap(), departure_product_type, 1, self.now);
            }
            if p > 0.0 && !is_continuing {
                p *= self.store.borrow_mut().before_probability(from_arrival, from_product_type, false, departure.unwrap(), departure_product_type, transfer_time, self.now);
            }
            if p > 0.0 {
                instr.selected_count += 1;
                new_distribution.add_with(dest_arr_dist.as_ref().unwrap(), p*remaining_probability, self.mean_only);
                remaining_probability = (1.0-p).clamp(0.0,1.0)*remaining_probability;
                last_departure = departure;
                last_product_type = departure_product_type;
                if remaining_probability <= self.epsilon {
                    break;
                }
            }
        }
        new_distribution.feasible_probability = (1.0-remaining_probability).clamp(0.0,1.0);
        if new_distribution.feasible_probability < 1.0 {
            new_distribution.normalize_with(self.mean_only);
        }
        instr.new_dist_time += start_ts.elapsed().as_nanos();
    }

    pub fn relevant_stations(&mut self, start_time: types::Mtime, origin_idx: usize, destination_idx: usize, station_labels: &[Vec<ConnectionLabel>]) -> HashMap<usize, f32> {
        let origin = connection::StopInfo {
            scheduled: start_time,
            delay: None,
            scheduled_track: "".to_string(),
            projected_track: "".to_string()
        };
        println!("from {} {} to {} {}", origin_idx, self.stations[origin_idx].name, destination_idx, self.stations[destination_idx].name);
        let mut stack = vec![(self.order[self.stations[origin_idx].arrivals[0]], 1.0)];
        println!("starting: {}", self.stations[self.connections[stack[0].0].to_idx].name);
        let mut weights_by_station_idx: HashMap<usize, f32> = HashMap::new();
        'outer: while !stack.is_empty() {
            let conn_with_prob = stack.pop().unwrap();
            let c = &self.connections[conn_with_prob.0];
            let station_idx = c.to_idx;
            if station_idx == destination_idx {
                weights_by_station_idx.insert(station_idx, 1000.0);
                continue;
            }

            let mut departures = vec![&station_labels[station_idx]];
            let mut transfer_times = vec![1];
            let footpaths = &self.stations[station_idx].footpaths;
            for i in 0..footpaths.len() {
                let stop_idx = footpaths[i].target_location_idx;
                if stop_idx == destination_idx {
                    for i in 0..footpaths.len() {
                        weights_by_station_idx.insert(footpaths[i].target_location_idx, 1000.0);
                    }
                    continue 'outer;
                }
                let transfer_time = footpaths[i].duration as i32;
                departures.push(&station_labels[stop_idx]);
                transfer_times.push(transfer_time);
            }
            let mut is = vec![0; transfer_times.len()];
            let mut remaining_probability = 1.0;
            while remaining_probability > self.epsilon {
                let mut min_mean = 1440.0*100.0;
                let mut min_k = 0;
                let mut found = false;
                for k in 0..departures.len() {
                    if is[k] < departures[k].len() {
                        let cand = departures[k][departures[k].len()-is[k]-1].destination_arrival.mean;
                        if cand < min_mean {
                            min_mean = cand;
                            min_k = k;
                            found = true;
                        }
                    }
                }
                if !found {
                    break;
                }
                let dep_label = &departures[min_k][departures[min_k].len()-is[min_k]-1];
                let mut p: f32 = dep_label.destination_arrival.feasible_probability;
                let dep = &self.connections[dep_label.connection_idx];
                is[min_k] += 1;

                if self.cut.contains(&(c.id, dep.id)) {
                    continue;
                }
                if station_idx == origin_idx {
                    p *= self.store.borrow_mut().before_probability(&origin, 100, false, &dep.departure, dep.product_type, transfer_times[min_k], self.now);                            
                } else if p > 0.0 && !c.is_consecutive(dep) {
                    p *= self.store.borrow_mut().before_probability(&c.arrival, c.product_type, false, &dep.departure, dep.product_type, transfer_times[min_k], self.now);
                }
                if p <= self.epsilon {
                    continue;
                }
                let dep_prob = p*remaining_probability*conn_with_prob.1;
                if !c.is_consecutive(dep) || station_idx == origin_idx {
                    let mut w = *weights_by_station_idx.get(&dep.from_idx).unwrap_or(&0.0);
                    w += dep_prob;
                    weights_by_station_idx.insert(dep.from_idx, w);
                    //println!("{} {} {}", dep.from_idx, self.stations[dep.from_idx].name, w);
                    if station_idx != dep.from_idx {
                        let mut w = *weights_by_station_idx.get(&station_idx).unwrap_or(&0.0);
                        w += dep_prob;
                        weights_by_station_idx.insert(station_idx, w);    
                        //println!("{} {} {}", station_idx, self.stations[station_idx].name, w);

                    }
                }
                if dep_prob > self.epsilon*self.epsilon {
                    stack.push((dep_label.connection_idx, dep_prob));
                }
                remaining_probability = (1.0-p).clamp(0.0,1.0)*remaining_probability;
            }
        }
        for w in &weights_by_station_idx {
            println!("{} {} {}", w.0, self.stations[*w.0].name, w.1);

        }
        weights_by_station_idx
    }

    pub fn relevant_connection_pairs(&mut self, weights_by_station_idx: HashMap<usize, f32>) -> HashMap<usize, usize> {
        let mut stations: Vec<(usize, f32)> = weights_by_station_idx.into_iter().collect();
        stations.sort_unstable_by(|a,b| b.1.partial_cmp(&a.1).unwrap());
        let mut trip_id_to_conn_idxs: HashMap<i32, Vec<(usize, bool)>> = HashMap::new();
        for i in 0..std::cmp::min(stations.len(), 500) {
            for arr in &self.stations[stations[i].0].arrivals {
                self.insert_relevant_conn_idx(arr, &mut trip_id_to_conn_idxs, false);
            }
            for dep in &self.stations[stations[i].0].departures {
                self.insert_relevant_conn_idx(dep, &mut trip_id_to_conn_idxs, true);
            }
        }
        let mut connection_pairs = HashMap::new();
        for trip in trip_id_to_conn_idxs.values_mut() {
            trip.sort_unstable_by(|a,b| self.connections[a.0].departure.scheduled.cmp(&self.connections[b.0].departure.scheduled)
                .then(self.connections[a.0].id.cmp(&self.connections[b.0].id))
                .then(b.1.cmp(&a.1))
            );
            let mut i = if !trip[0].1 { 1 } else { 0 };
            while i+1 < trip.len() {
                connection_pairs.insert(trip[i+1].0, trip[i].0);
                i += 2;
            }
        }
        connection_pairs
    }

    fn insert_relevant_conn_idx(&mut self, conn_id: &usize, trip_id_to_conn_idxs: &mut HashMap<i32, Vec<(usize, bool)>>, is_departure: bool) {
        let connidx = self.order[*conn_id];
        let c = &self.connections[connidx];
        match trip_id_to_conn_idxs.get_mut(&c.trip_id) {
            Some(list) => list.push((connidx, is_departure)),
            None => {
                trip_id_to_conn_idxs.insert(c.trip_id, vec![(connidx, is_departure)]);
            }
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
        let stations = vec![s];
        let mut connections: Vec<connection::Connection> = vec![];

        let cut = prepare_and_query(&mut store, &mut connections, &stations, &stations[0], &stations[0], 0, 0, 0, 0.0, false);
        assert_eq!(cut.len(), 0);
    }
}