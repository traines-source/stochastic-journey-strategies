use std::cell::RefCell;
use std::collections::HashMap;
use std::collections::HashSet;
use std::hash::Hash;
use std::time::Instant;

use indexmap::IndexMap;
use motis_nigiri::EventChange;
use serde::Deserialize;
use serde::Serialize;

use crate::distribution;
use crate::distribution_store;
use crate::connection;
use crate::types;

pub fn new<'a, 'b>(store: &'b mut distribution_store::Store, connections: &'b mut Vec<connection::Connection>, stations: &'b [connection::Station], cut: HashSet<(usize, usize)>, order: &'b mut HashMap<usize, ConnectionOrder>, now: types::Mtime, epsilon: f32, mean_only: bool) -> Environment<'b> {
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

pub fn prepare<'a, 'b>(store: &'b mut distribution_store::Store, connections: &'b mut Vec<connection::Connection>, stations: &'b [connection::Station], order: &'b mut HashMap<usize, ConnectionOrder>, now: types::Mtime, epsilon: f32, mean_only: bool) -> Environment<'b> {
    let mut e = new(store, connections, stations, HashSet::new(), order, now, epsilon, mean_only);
    println!("Starting topocsa...");
    e.preprocess();
    e
}

pub fn prepare_and_query<'a, 'b>(store: &'b mut distribution_store::Store, connections: &'b mut Vec<connection::Connection>, stations: &'b [connection::Station], origin: &'a connection::Station, destination: &'a connection::Station, _start_time: types::Mtime, _max_time: types::Mtime, now: types::Mtime, epsilon: f32, mean_only: bool) -> HashSet<(usize, usize)>  {
    let mut order = HashMap::with_capacity(connections.len());
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
    pub cut: HashSet<(usize, usize)>,
    order: &'b mut HashMap<usize, ConnectionOrder>
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ConnectionOrder {
    visited: i16,
    pub order: usize
}

pub struct ConnectionLabel {
    pub connection_idx: usize,
    pub destination_arrival: distribution::Distribution
}

impl<'a, 'b> Environment<'b> {

    fn dfs(&mut self, anchor_id: usize, topo_idx: &mut usize, max_stack: &mut usize, max_trace: &mut usize) {
        let mut unraveling_time = 0;
        let mut unraveling_no = 0;
        let mut stack: Vec<usize> = Vec::with_capacity(10000);
        let mut trace: IndexMap<usize, usize> = IndexMap::with_capacity(1000);
        stack.push(anchor_id);
        self.order.insert(anchor_id, ConnectionOrder{visited: 0, order: 0});
        while !stack.is_empty() {
            let c_id = *stack.last().unwrap();
            let c = &self.connections[c_id];
            let c_label = self.order.get_mut(&c_id).unwrap();
            if c_label.visited == 0 {
                c_label.visited = 1;
                trace.insert(c_id, stack.len()-1);
            } else {
                if c_label.visited == 1 {
                    c_label.order = *topo_idx;
                    *topo_idx += 1;
                    let p = trace.pop().unwrap();
                    assert_eq!(p.0, c_id);
                    assert_eq!(p.1, stack.len()-1);
                } else {
                    assert_eq!(c_label.visited, 2);
                }
                c_label.visited = 2;
                stack.pop();
                continue;
            }
            let mut i = 0;
            let footpaths = &self.stations[c.to_idx].footpaths;
            'outer: while i <= footpaths.len() {
                let station_idx = if i == footpaths.len() { c.to_idx } else { footpaths[i].target_location_idx };
                let transfer_time = if i == footpaths.len() { 1 } else { footpaths[i].duration as i32 };
                let deps = self.stations[station_idx].departures.borrow();
                for dep_id in &*deps {
                    if self.cut.contains(&(c_id, *dep_id)) {
                        continue;
                    }
                    let dep = self.connections.get(*dep_id).unwrap();
                    let is_continuing = if i == footpaths.len() { c.is_consecutive(dep) } else { false };
                    // TODO max reachability independent from now
                    if !is_continuing {
                        let reachable = self.store.borrow_mut().before_probability(&c.arrival, c.product_type, false, &dep.departure, dep.product_type, transfer_time, self.now);
                        if reachable <= self.epsilon {
                            continue;
                        }
                    }
                    let dep_label = self.order.get(dep_id);
                    if dep_label.is_some() {
                        let dep_label = dep_label.unwrap();
                        if dep_label.visited == 1 {
                            let trace_idx = trace.get_index_of(dep_id);
                            if trace_idx.is_some() {
                                let start_ts = Instant::now();
                                let transfer_time = dep.departure.projected()-c.arrival.projected();
                                let mut min_transfer = if c.is_consecutive(dep) { 1 } else { transfer_time };
                                let mut min_i = trace.len();
                                let start = trace_idx.unwrap()+1 as usize;
                                for i in start..trace.len() {
                                    let a = &self.connections[*trace.get_index(i-1).unwrap().0];
                                    let b = &self.connections[*trace.get_index(i).unwrap().0];
                                    if a.is_consecutive(b) {
                                        continue;
                                    }
                                    let t = b.departure.projected()-a.arrival.projected();
                                    if t < min_transfer {
                                        min_transfer = t;
                                        min_i = i;
                                    }
                                }
                                if min_transfer > 0 {
                                    panic!("cutting positive transfer {:?} {:?} {} {}", c.departure, c.route_idx, min_transfer, transfer_time)
                                }
                                if min_i == trace.len() {
                                    self.cut.insert((c_id, *dep_id));
                                    if c.is_consecutive(dep) {
                                        panic!("cutting trip"); 
                                    }
                                    continue;
                                }
                                let cut_before = trace.get_index(min_i).unwrap();
                                let cut_after = trace.get_index(min_i-1).unwrap();
                                self.cut.insert((*cut_after.0, *cut_before.0));
                                if self.connections[*cut_after.0].is_consecutive(&self.connections[*cut_before.0]) {
                                    panic!("cutting trip {:?} {:?}", self.connections[*cut_after.0],self.connections[*cut_before.0]);
                                }
                                unraveling_no += stack.len()-*cut_before.1;
                                stack.truncate(*cut_before.1);
                                for _ in min_i..trace.len() {
                                    let l = self.order.get_mut(&trace.pop().unwrap().0).unwrap();
                                    assert_eq!(l.visited, 1);
                                    l.visited = 0;
                                }
                                unraveling_time += start_ts.elapsed().as_micros();
                                break 'outer;
                            } else {
                                panic!("marked as visited but not in trace {:?} {:?}", *dep_id, trace);
                            }
                        } else if dep_label.visited == 2 {
                            continue;
                        }
                    }
                    stack.push(*dep_id);
                    self.order.insert(*dep_id, ConnectionOrder { visited: 0, order: 0 });
                }
                i += 1;
            }
            if stack.len() > *max_stack {
                *max_stack = stack.len();
            }
            if trace.len() > *max_trace {
                *max_trace = trace.len();
            }
        }
        println!("max stack {} trace {} unraveling: {} {}", max_stack, max_trace, unraveling_time, unraveling_no);
    }
    
    pub fn preprocess(&mut self) {
        println!("Start preprocessing...");
        let mut topo_idx = 0;
        
        let mut max_stack = 0;
        let mut max_trace = 0;
        let start = Instant::now();
        for i in 0..self.connections.len() {
            if !self.order.contains_key(&i) || self.order.get(&i).unwrap().visited != 2 {
                self.dfs(i, &mut topo_idx, &mut max_stack, &mut max_trace);
                println!("connections {} cycles found {} labels {} done {}", self.connections.len(), self.cut.len(), self.order.len(), i);
            }
        }
        println!("Done DFSing. {}", start.elapsed().as_millis());
        self.connections.sort_by(|a, b|
            self.order.get(&a.id).unwrap().order.partial_cmp(&self.order.get(&b.id).unwrap().order).unwrap()
        );
        println!("Done preprocessing.");
        println!("cut: {}", self.cut.len());
    }

    pub fn update(&mut self, connection_id: usize, is_departure: bool, delay: i16, cancelled: bool) {
        match self.order.get(&connection_id) {
            Some(order) => {
                let c = &mut self.connections[order.order];
                if cancelled {
                    c.cancelled = true;            
                } else if is_departure {
                    c.departure.delay = Some(delay);
                } else {
                    c.arrival.delay = Some(delay);
                }
            },
            None => println!("Failed to find matching connection for update.")
        }
    }
    pub fn query(&mut self, _origin: &'a connection::Station, destination: &'a connection::Station) -> HashMap<usize, Vec<ConnectionLabel>> {
        let pairs = HashMap::new();
        self.pair_query(_origin, destination, &pairs)
    }

    pub fn pair_query(&mut self, _origin: &'a connection::Station, destination: &'a connection::Station, connection_pairs: &HashMap<usize, usize>) -> HashMap<usize, Vec<ConnectionLabel>> {
        let mut station_labels: HashMap<usize, Vec<ConnectionLabel>> = HashMap::new();
        let empty_vec = vec![];
        for i in 0..self.connections.len() {
            if connection_pairs.len() > 0 && !connection_pairs.contains_key(&i) {
                continue;
            }
            let c = self.connections.get(i).unwrap();
            if !station_labels.contains_key(&c.to_idx) {
                station_labels.insert(c.to_idx, vec![]);
            }
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
                        if !station_labels.contains_key(&f.target_location_idx) {
                            station_labels.insert(f.target_location_idx, vec![]);
                        }
                        self.new_destination_arrival(f.target_location_idx, 0, -1, 0, c.product_type, &c.arrival, f.duration as i32, &station_labels, &empty_vec, &mut footpath_dest_arr);   
                    }
                    if footpath_dest_arr.feasible_probability > 0.0 {
                        footpath_distributions.push(footpath_dest_arr);
                    }
                }
                //println!("{:?} {:?}", footpath_distributions.len(), footpaths.len());
                footpath_distributions.sort_by(|a, b| a.mean.partial_cmp(&b.mean).unwrap());
                self.new_destination_arrival(c.to_idx, c.id, c.trip_id, c.route_idx, c.product_type, &c.arrival, 1, &station_labels, &footpath_distributions, &mut new_distribution);   
            }
            let departure_conn_idx = if connection_pairs.len() == 0 { i } else { connection_pairs[&i] };
            let departure_conn = if connection_pairs.len() == 0 { c } else { &self.connections[connection_pairs[&i]] };
            let departure_station_idx = departure_conn.from_idx;
            if !station_labels.contains_key(&departure_station_idx) {
                station_labels.insert(departure_station_idx, vec![]);
            }
            let station_label = station_labels.get_mut(&departure_station_idx);
            let departures = station_label.unwrap();
            departure_conn.destination_arrival.replace(Some(new_distribution.clone())); // TODO remove
            if new_distribution.feasible_probability > 0.0 {
                // TODO pareto? - sort incoming departures and connections by dep?
                let mut j = departures.len() as i32-1;
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
                        do_insert = false;
                    }
                }
                if do_insert {
                    departures.insert((j+1) as usize, ConnectionLabel{connection_idx: departure_conn_idx, destination_arrival: new_distribution});
                }
            }
        }
        station_labels
    }

    fn new_destination_arrival<'c>(&'c self, station_idx: usize, c_id: usize, from_trip_id: i32, from_route_idx: usize, from_product_type: i16, from_arrival: &connection::StopInfo, transfer_time: i32, station_labels: &HashMap<usize, Vec<ConnectionLabel>>, footpath_distributions: &[distribution::Distribution], new_distribution: &mut distribution::Distribution) {
        let mut remaining_probability = 1.0;
        let mut last_departure: Option<&connection::StopInfo> = None;
        let mut last_product_type: i16 = 0;
        let departures = station_labels.get(&station_idx).unwrap();

        let mut departures_i = 0;
        let mut footpaths_i = 0;
        while departures_i < departures.len() || footpaths_i < footpath_distributions.len() {
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
            if departures_i < departures.len() {
                let dep_i = departures.len()-1-departures_i;
                let dep = self.connections.get(departures[dep_i].connection_idx).unwrap();
                if self.cut.contains(&(c_id, dep.id)) {
                    departures_i += 1;
                    continue;
                }
                dest = Some(&departures[dep_i].destination_arrival);
                let candidate = dest.unwrap();
                if dest_arr_dist.is_some_and(|d| candidate.mean > d.mean) {
                    footpaths_i += 1;
                } else {
                    departures_i += 1;
                    dest_arr_dist = Some(candidate);
                    departure = Some(&dep.departure);
                    departure_product_type = dep.product_type;
                    is_continuing = from_trip_id == dep.trip_id && from_route_idx == dep.route_idx && from_arrival.scheduled <= dep.departure.scheduled
                }
            } else {
                footpaths_i += 1;
            }
            let mut p: f32 = dest_arr_dist.unwrap().feasible_probability;
            if p <= self.epsilon {
                continue;
            }
            if expect_float_absolute_eq!(dest_arr_dist.unwrap().mean, 0.0, 1e-3).is_ok() {
                panic!("mean 0 with high feasibility");
            }
            //assert_float_absolute_eq!(dest.as_ref().unwrap().mean, dest.as_ref().unwrap().mean(), 1e-3);
            /*if last_departure.is_some() {
                p *= self.store.borrow_mut().before_probability(last_departure.unwrap(), last_product_type, true, departure.unwrap(), departure_product_type, 1, self.now);
            }*/
            if p > 0.0 && !is_continuing {
                p *= self.store.borrow_mut().before_probability(from_arrival, from_product_type, false, departure.unwrap(), departure_product_type, transfer_time, self.now);
            }
            if p > 0.0 {
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
    }

    pub fn relevant_stations(&mut self, start_time: types::Mtime, origin_idx: usize, destination_idx: usize, station_labels: &HashMap<usize, Vec<ConnectionLabel>>) -> HashMap<usize, f32> {
        let origin = connection::StopInfo {
            scheduled: start_time,
            delay: None,
            scheduled_track: "".to_string(),
            projected_track: "".to_string()
        };
        println!("from {} {} to {} {}", origin_idx, self.stations[origin_idx].name, destination_idx, self.stations[destination_idx].name);
        let mut stack = vec![(self.order[&self.stations[origin_idx].arrivals[0]].order, 1.0)];
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

            let mut departures = vec![&station_labels[&station_idx]];
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
                departures.push(&station_labels[&stop_idx]);
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
        stations.sort_by(|a,b| b.1.partial_cmp(&a.1).unwrap());
        let mut trip_id_to_conn_idxs: HashMap<i32, Vec<(usize, bool)>> = HashMap::new();
        for i in 0..std::cmp::min(stations.len(), 500) {
            for arr in &self.stations[stations[i].0].arrivals {
                self.insert_relevant_conn_idx(arr, &mut trip_id_to_conn_idxs, false);
            }
            for dep in &*self.stations[stations[i].0].departures.borrow() {
                self.insert_relevant_conn_idx(dep, &mut trip_id_to_conn_idxs, true);
            }
        }
        let mut connection_pairs = HashMap::new();
        for trip in trip_id_to_conn_idxs.values_mut() {
            trip.sort_by(|a,b| self.connections[a.0].departure.scheduled.cmp(&self.connections[b.0].departure.scheduled)
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
        let connidx = self.order[conn_id].order;
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