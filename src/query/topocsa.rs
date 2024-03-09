use std::cell::RefCell;
use std::collections::HashMap;
use std::collections::HashSet;
use std::time::Instant;
use rustc_hash::FxHashSet;


use indexmap::IndexSet;
use serde::Deserialize;
use serde::Serialize;

use crate::distribution;
use crate::distribution_store;
use crate::connection;
use crate::gtfs::StationContraction;
use crate::types;

pub fn new<'a>(store: &'a mut distribution_store::Store, connections: &'a mut Vec<connection::Connection>, stations: &'a [connection::Station], cut: &'a mut FxHashSet<(usize, usize)>, order: &'a mut Vec<usize>, now: types::Mtime, epsilon_reachable: f32, epsilon_feasible: f32, mean_only: bool, domination: bool) -> Environment<'a> {
    if order.is_empty() {
        order.extend(0..connections.len());
    }
    Environment {
        store: RefCell::new(store),
        connections: connections,
        stations: stations,
        now,
        epsilon_reachable,
        epsilon_feasible,
        mean_only,
        domination,
        cut,
        order,
        contraction: None
    }
}

pub fn prepare<'a>(store: &'a mut distribution_store::Store, connections: &'a mut Vec<connection::Connection>, stations: &'a [connection::Station], cut: &'a mut FxHashSet<(usize, usize)>, order: &'a mut Vec<usize>, now: types::Mtime, epsilon: f32, mean_only: bool) -> Environment<'a> {
    let mut e = new(store, connections, stations, cut, order, now, epsilon, epsilon, mean_only, false);    
    println!("Starting topocsa...");
    e.preprocess();
    e
}

pub fn prepare_and_query<'a>(store: &'a mut distribution_store::Store, connections: &'a mut Vec<connection::Connection>, stations: &'a [connection::Station], cut: &'a mut FxHashSet<(usize, usize)>, origin: usize, destination: usize, start_time: types::Mtime, max_time: types::Mtime, now: types::Mtime, epsilon: f32, mean_only: bool) {
    let mut order = Vec::with_capacity(connections.len());
    let mut e = prepare(store, connections, stations, cut, &mut order, now, epsilon, mean_only);
    e.query(origin, destination, start_time, max_time);
    println!("Done.");
}

#[derive(Debug)]
pub struct Environment<'a> {
    store: RefCell<&'a mut distribution_store::Store>,
    connections: &'a mut Vec<connection::Connection>,
    stations: &'a [connection::Station],
    now: types::Mtime,
    epsilon_reachable: f32,
    epsilon_feasible: f32,
    mean_only: bool,
    domination: bool,
    cut: &'a mut FxHashSet<(usize, usize)>,
    order: &'a mut Vec<usize>,
    contraction: Option<&'a StationContraction>
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DfsConnectionLabel {
    footpath_i: usize,
    i: usize,
    order: usize
}

pub struct ConnectionLabel {
    pub connection_idx: usize,
    pub destination_arrival: distribution::Distribution,
    pub prob_after: f32,
    pub departure_mean: f32
}

#[derive(Debug)]
pub struct Instrumentation {
    found: usize,
    encounter_1: usize,
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
    looked_at: usize,
    deps: usize,
    selected_count: usize,
    infeas_iter: usize,
}

impl<'a> Environment<'a> {

    pub fn set_station_contraction(&mut self, contr: &'a StationContraction) {
        self.contraction = Some(contr);
    }

    fn dfs(&mut self, anchor_idx: usize, topo_idx: &mut usize, labels: &mut Vec<DfsConnectionLabel>, visited: &mut Vec<i16>, stops_completed_up: &mut Vec<usize>, instr: &mut Instrumentation) {
        let mut stack: Vec<usize> = Vec::with_capacity(1000);
        stack.push(anchor_idx);
        while !stack.is_empty() {
            instr.iterations += 1;
            let c_idx = *stack.last().unwrap();
            let c = &self.connections[c_idx];
            let c_label = labels.get_mut(c_idx).unwrap();
            let footpaths = &self.stations[c.to_idx].footpaths;
            let mut stop_idx = if c_label.footpath_i == footpaths.len() { c.to_idx } else { footpaths[c_label.footpath_i].target_location_idx };
            let mut deps = &self.stations[stop_idx].departures;
            let mut streak = false;
            if c_label.i >= stops_completed_up[stop_idx] {
                c_label.i = stops_completed_up[stop_idx];
                streak = true;
            }
            visited[c_idx] = 1;
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
                let dep_idx = self.order[deps[c_label.i]];
                let dep_visited = visited[dep_idx];
                if dep_visited == 2 {
                    instr.encounter_2 += 1;
                } else {
                    if streak {
                        stops_completed_up[stop_idx] = c_label.i+1;
                        streak = false;
                    }
                    found = true;
                    instr.found += 1;
                    let dep = &self.connections[dep_idx];
                    let is_continuing = if c_label.footpath_i == footpaths.len() { c.is_consecutive(dep) } else { false };
                    if !is_continuing {
                        let transfer_time = if c_label.footpath_i == footpaths.len() { self.stations[stop_idx].transfer_time } else { footpaths[c_label.footpath_i].duration } as i32;
                        let reachable = self.store.borrow_mut().before_probability(&c.arrival, c.product_type, false, &dep.departure, dep.product_type, transfer_time, self.now);
                        if reachable <= self.epsilon_reachable {
                            if reachable == 0.0 {
                                let diff = (dep.departure.projected()-c.arrival.projected()-transfer_time) as i16;
                                if diff < self.store.borrow().min_delay_diff {
                                    c_label.i = 0;
                                }
                            }
                            continue;
                        }
                    }
                    
                    if dep_visited == 1 {
                        instr.encounter_1 += 1;
                        let predicted_transfer_time = dep.departure.projected()-c.arrival.projected();
                        let mut min_transfer = if c.is_consecutive(dep) { 1 } else { predicted_transfer_time };
                        let mut min_i = stack.len();
                        let mut i = stack.len();
                        while stack[i-1] != dep_idx {
                            i -= 1;
                            let a = &self.connections[stack[i-1]];
                            let b = &self.connections[stack[i]];
                            if a.is_consecutive(b) {
                                continue;
                            }
                            let predicted_transfer_time = b.departure.projected()-a.arrival.projected();
                            if predicted_transfer_time < min_transfer {
                                min_transfer = predicted_transfer_time;
                                min_i = i;
                            }
                        }
                        instr.cycle_sum_len += stack.len()-i;
                        if stack.len()-i > instr.cycle_max_len {
                            instr.cycle_max_len = stack.len()-i;
                        }
                        if min_i == stack.len() {
                            if self.epsilon_reachable == 0.0 {
                                self.cut.insert((c.id, dep.id));
                            }
                            if c.id == dep.id {
                                instr.cycle_self_count += 1;
                            }
                            continue;
                        }
                        if self.epsilon_reachable == 0.0 {
                            let cut_predecessor = stack[min_i-1];
                            let cut_successor = stack[min_i];
                            self.cut.insert((cut_predecessor, cut_successor));
                        }
                        instr.unraveling_no += stack.len()-min_i;
                        let cut_len = min_i..stack.len();
                        for _ in cut_len {
                            let idx = stack.pop().unwrap();
                            let label = labels.get_mut(idx).unwrap();
                            label.i += 1;
                            visited[idx] = 0;
                        }
                        break;
                    } else if dep_visited != 0 {
                        panic!("unexpected visited state");
                    }
                    stack.push(dep_idx);
                    break;
                }
            }
            if !found {
                let c_label = labels.get_mut(c_idx).unwrap();
                assert_eq!(c_label.i, 0);
                if streak {
                    stops_completed_up[stop_idx] = 0;
                }
                c_label.order = *topo_idx;
                *topo_idx += 1;
                visited[c_idx] = 2;
                let p = stack.pop().unwrap();
                assert_eq!(p, c_idx);
            }
        }
        //println!("instr: {:?}", instr);
    }
    
    pub fn preprocess(&mut self) {
        self.cut.clear();
        println!("Start preprocessing...");
        let mut conn_idxs: Vec<usize> = (0..self.connections.len()).collect();
        conn_idxs.sort_unstable_by(|a,b| self.connections[*a].departure.projected().cmp(&self.connections[*b].departure.projected()));

        let mut topo_idx = 0;
        
        let mut instr = Instrumentation { found: 0, encounter_1: 0, unraveling_time: 0, before_prob_time: 0, unraveling_no: 0, cycle_sum_len: 0, cycle_max_len: 0, cycle_self_count: 0, encounter_2: 0, iterations: 0 };
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
        for i in 0..self.connections.len() {
            let idx = conn_idxs[i];
            if visited[idx] != 2 {
                self.dfs(idx, &mut topo_idx, &mut labels, &mut visited, &mut stops_completed_up, &mut instr);
                //println!("connections {} cycles found {} labels {} done {} {}", self.connections.len(), self.cut.len(), self.order.len(), idx, id);
            }
        }
        println!("instr: {:?}", instr);
        self.store.borrow().print_stats();
        println!("Done DFSing. {}", start.elapsed().as_millis());
        self.connections.sort_unstable_by(|a, b|
            labels[self.order[a.id]].order.partial_cmp(&labels[self.order[b.id]].order).unwrap()
        );
        let mut new_order: Vec<usize> = (0..self.connections.len()).map(|id| labels[self.order[id]].order).collect();
        self.order.clear();
        self.order.append(&mut new_order);
        println!("Done preprocessing.");
        println!("connections: {} topoidx: {} cut: {}", self.connections.len(), topo_idx, self.cut.len());
    }

    pub fn update(&mut self, connection_id: usize, is_departure: bool, location_idx: Option<usize>, in_out_allowed: Option<bool>, delay: Option<i16>) {
        let c = &mut self.connections[self.order[connection_id]];
        if location_idx.is_some() {
            //println!("Platform change is_dep: {} new_location: {} c: {:?}", is_departure, location_idx.unwrap(), c);
            if is_departure {
                c.from_idx = location_idx.unwrap();
            } else {
                c.to_idx = location_idx.unwrap();
            }
        }
        if in_out_allowed.is_some() {
            //println!("In_out_allowed change is_dep: {} new_location: {} c: {:?}", is_departure, in_out_allowed.unwrap(), c);
            if is_departure {
                c.departure.in_out_allowed = in_out_allowed.unwrap();
            } else {
                c.arrival.in_out_allowed = in_out_allowed.unwrap();
            }           
        }
        if delay.is_some() {
            if is_departure {
                c.departure.delay = delay;
            } else {
                c.arrival.delay = delay;
            }
        }
    }

    pub fn query(&mut self, _origin: usize, destination: usize, start_time: types::Mtime, max_time: types::Mtime) -> Vec<Vec<ConnectionLabel>> {
        let pairs = HashMap::new();
        let start_ts = Instant::now();
        let r = self.pair_query(_origin, destination, start_time, max_time, &pairs);
        println!("elapsed: {}", start_ts.elapsed().as_millis());
        r
    }

    pub fn pair_query(&mut self, _origin: usize, destination: usize, start_time: types::Mtime, max_time: types::Mtime, connection_pairs: &HashMap<i32, i32>) -> Vec<Vec<ConnectionLabel>> {
        let mut connection_pair_ids = vec![-1; if connection_pairs.len() > 0 { self.connections.len() } else { 0 }];
        for pair in connection_pairs.iter() {
            connection_pair_ids[self.order[*pair.0 as usize]] = *pair.1;
        }
        let mut instr = CsaInstrumentation {
            looked_at: 0,
            deps: 0,
            selected_count: 0,
            infeas_iter: 0
        };
        let mut station_labels: Vec<Vec<ConnectionLabel>> = (0..self.stations.len()).map(|i| Vec::new()).collect();
        let empty_vec = vec![];
        let max_delay = self.store.borrow().max_delay as types::Mtime;
        for i in 0..self.connections.len() {
            if connection_pair_ids.len() > 0 && connection_pair_ids[i] == -1 {
                continue;
            }
            let c = &self.connections[i];

            if c.departure.projected()+max_delay < start_time || c.departure.projected() >= max_time {
                continue;
            }
            instr.looked_at += 1;
            let stop_idx = match self.contraction {
                Some(contr) => contr.stop_to_group[c.to_idx],
                None => c.to_idx
            };
            let dest_contr = match self.contraction {
                Some(contr) => contr.stop_to_group[destination],
                None => destination
            };
            /*let orig_contr = match self.contraction {
                Some(contr) => contr.stop_to_group[origin],
                None => destination
            };
            if stop_idx == orig_contr && c.arrival.projected()+max_delay < start_time {
                break;
            }*/
            let new_distribution = if stop_idx == dest_contr {
                if !c.arrival.in_out_allowed {
                    if !self.mean_only {
                        c.destination_arrival.replace(Some(distribution::Distribution::empty(c.arrival.scheduled))); //TODO remove
                    }
                    continue;
                }
                let mut new_distribution = self.store.borrow().delay_distribution(&c.arrival, false, c.product_type, self.now);
                if c.to_idx != destination {
                    let contr = self.contraction.unwrap();
                    new_distribution = new_distribution.shift(contr.get_transfer_time(c.to_idx, destination) as i32);
                }
                new_distribution
            } else {
                let mut new_distribution = distribution::Distribution::empty(c.arrival.scheduled);
                if self.contraction.is_none() {
                    let mut footpath_distributions = vec![];
                    let footpaths = &self.stations[stop_idx].footpaths;
                    for f in footpaths {
                        let mut footpath_dest_arr = distribution::Distribution::empty(0);
                        if f.target_location_idx == destination {
                            if !c.arrival.in_out_allowed {
                                if !self.mean_only {
                                    c.destination_arrival.replace(Some(distribution::Distribution::empty(c.arrival.scheduled))); //TODO remove
                                }
                                continue;
                            }
                            footpath_dest_arr = self.store.borrow().delay_distribution(&c.arrival, false, c.product_type, self.now).shift(f.duration as i32);
                        } else {
                            self.new_destination_arrival(f.target_location_idx, i, -1, 0, c.product_type, &c.arrival, f.duration as i32, &station_labels, &empty_vec, &mut footpath_dest_arr, &mut instr);   
                        }
                        if footpath_dest_arr.feasible_probability > 0.0 {
                            footpath_distributions.push(footpath_dest_arr);
                        }
                    }
                    //println!("{:?} {:?}", footpath_distributions.len(), footpaths.len());
                    // TODO domination in case of strict domination
                    footpath_distributions.sort_unstable_by(|a, b| a.mean.partial_cmp(&b.mean).unwrap());
                    self.new_destination_arrival(stop_idx, i, c.trip_id, c.route_idx, c.product_type, &c.arrival, self.stations[stop_idx].transfer_time as i32, &station_labels, &footpath_distributions, &mut new_distribution, &mut instr);   
                } else {
                    if station_labels[stop_idx].is_empty() {
                        continue;
                    } 
                    self.new_contr_destination_arrival(stop_idx, i, &station_labels, &mut new_distribution, &mut instr);   
                }
                new_distribution   
            };

            let departure_conn_idx = if connection_pair_ids.len() == 0 { i } else { self.order[connection_pair_ids[i] as usize] };
            let departure_conn = if connection_pair_ids.len() == 0 { c } else { &self.connections[departure_conn_idx] };
            let departure_station_idx = match self.contraction {
                Some(contr) => contr.stop_to_group[departure_conn.from_idx],
                None => departure_conn.from_idx
            };
            let departures = station_labels.get_mut(departure_station_idx).unwrap();
            if !self.mean_only {
                departure_conn.destination_arrival.replace(Some(new_distribution.clone())); // TODO remove
            }
            if new_distribution.feasible_probability > self.epsilon_feasible {
                let mut j = departures.len() as i32-1;
                while j >= 0 {
                    if new_distribution.mean < departures[j as usize].destination_arrival.mean {
                        break;
                    }
                    j -= 1;
                }
                let mut prob_after = 1.0;
                let mut departure_mean = 0.0;

                if self.domination {
                    departure_mean = self.store.borrow_mut().delay_distribution_mean(&departure_conn.departure, true, departure_conn.product_type, self.now);
                    if ((j+1) as usize) < departures.len() && departure_mean < departures[(j+1) as usize].departure_mean {
                        continue;
                    }
                    if j >= 0 && departure_mean > departures[j as usize].departure_mean {
                        let mut k = j-1;
                        while k >= 0 && departure_mean > departures[k as usize].departure_mean {
                            k -= 1;
                        }
                        let replace_up_to = (k+1) as usize;
                        if replace_up_to != j as usize {
                            departures.drain(replace_up_to..j as usize);
                        }
                        departures[replace_up_to] = ConnectionLabel{
                            connection_idx: departure_conn_idx,
                            destination_arrival: new_distribution,
                            prob_after: 1.0,
                            departure_mean: departure_mean
                        };
                        continue;
                    }
                } else if self.contraction.is_some() {
                    if ((j+1) as usize) < departures.len() { 
                        let ref_label = &departures[(j+1) as usize];
                        let reference = &self.connections[ref_label.connection_idx];
                        prob_after = self.store.borrow_mut().before_probability(&reference.departure, reference.product_type, true, &departure_conn.departure, departure_conn.product_type, 1, self.now)
                    }
                    if prob_after > 0.0 && j >= 0 {
                        let ref_label = departures.get_mut(j as usize).unwrap();
                        let reference = &self.connections[ref_label.connection_idx];
                        ref_label.prob_after = self.store.borrow_mut().before_probability(&departure_conn.departure, departure_conn.product_type, true, &reference.departure, reference.product_type, 1, self.now);
                    }
                }
                if prob_after > 0.0 {                    
                    departures.insert((j+1) as usize, ConnectionLabel{
                        connection_idx: departure_conn_idx,
                        destination_arrival: new_distribution,
                        prob_after: prob_after,
                        departure_mean: departure_mean
                    });
                }
            }
        }
        println!("instr {:?}", instr);
        self.store.borrow().print_stats();
        station_labels
    }

    #[inline]
    fn new_destination_arrival<'c>(&'c self, station_idx: usize, c_idx: usize, from_trip_id: i32, from_route_idx: usize, from_product_type: i16, from_arrival: &connection::StopInfo, transfer_time: i32, station_labels: &[Vec<ConnectionLabel>], footpath_distributions: &[distribution::Distribution], new_distribution: &mut distribution::Distribution, instr: &mut CsaInstrumentation) {
        let mut remaining_probability = 1.0;
        let mut last_departure: Option<&connection::StopInfo> = None;
        let mut last_product_type: i16 = 0;
        let departures = station_labels.get(station_idx).unwrap();

        let mut departures_i = 0;
        let mut footpaths_i = 0;
        let c = &self.connections[c_idx];
        while departures_i < departures.len() || footpaths_i < footpath_distributions.len() {
            let mut dest_arr_dist = None;
            let mut departure = None;
            let mut departure_product_type = 0;
            let mut is_continuing = false;
            let mut transfer_time = transfer_time;
            if footpaths_i < footpath_distributions.len() {
                dest_arr_dist = Some(&footpath_distributions[footpaths_i]);
                departure = Some(&c.arrival);
                departure_product_type = c.product_type;
                is_continuing = true;
            }
            if departures_i < departures.len() {
                let dep_i = departures.len()-1-departures_i;
                let label = &departures[dep_i];
                let dep = &self.connections[label.connection_idx];
                if self.cut.contains(&(c.id, dep.id)) {
                    departures_i += 1;
                    continue;
                }
                if dest_arr_dist.is_some_and(|d| label.destination_arrival.mean > d.mean) {
                    footpaths_i += 1;
                } else {
                    departures_i += 1;
                    dest_arr_dist = Some(&label.destination_arrival);
                    departure = Some(&dep.departure);
                    departure_product_type = dep.product_type;
                    is_continuing = from_trip_id == dep.trip_id && from_route_idx == dep.route_idx && from_arrival.scheduled <= dep.departure.scheduled && c.id != dep.id && c.to_idx == dep.from_idx;
                    transfer_time = match self.contraction {
                        Some(contr) => contr.get_transfer_time(c.to_idx, dep.from_idx) as i32,
                        None => transfer_time
                    }
                }
            } else {
                footpaths_i += 1;
            }
            instr.deps += 1;
            let mut p: f32 = dest_arr_dist.unwrap().feasible_probability;
            if !self.domination && last_departure.is_some() {
                p *= self.store.borrow_mut().before_probability(last_departure.unwrap(), last_product_type, true, departure.unwrap(), departure_product_type, 1, self.now);
            }
            if p > 0.0 && !is_continuing {
                p *= self.store.borrow_mut().before_probability(from_arrival, from_product_type, false, departure.unwrap(), departure_product_type, transfer_time, self.now);
            }
            if p > 0.0 {
                new_distribution.add_with(dest_arr_dist.as_ref().unwrap(), p*remaining_probability, self.mean_only);
                remaining_probability = (1.0-p).clamp(0.0,1.0)*remaining_probability;
                last_departure = departure;
                last_product_type = departure_product_type;
                if remaining_probability <= self.epsilon_feasible {
                    break;
                }
            }
        }
        new_distribution.feasible_probability = (1.0-remaining_probability).clamp(0.0,1.0);
        if new_distribution.feasible_probability < 1.0 {
            new_distribution.normalize_with(self.mean_only, self.epsilon_feasible*self.epsilon_feasible);
        }
    }

    #[inline]
    fn new_contr_destination_arrival<'c>(&'c self, station_idx: usize, c_idx: usize, station_labels: &[Vec<ConnectionLabel>], new_distribution: &mut distribution::Distribution, instr: &mut CsaInstrumentation) {
        let mut remaining_probability = 1.0;
        let departures = &station_labels[station_idx];
        let contr = self.contraction.unwrap();

        let c = &self.connections[c_idx];
        let mut store = self.store.borrow_mut();
        for dep_label in departures.iter().rev() {
            instr.deps += 1;
            let dep = &self.connections[dep_label.connection_idx];
            if self.cut.contains(&(c.id, dep.id)) {
                continue;
            }
            let mut p: f32 = dep_label.destination_arrival.feasible_probability*dep_label.prob_after;
            if !c.is_consecutive(dep) { 
                let transfer_time = contr.get_transfer_time(c.to_idx, dep.from_idx) as i32;
                p *= store.before_probability(&c.arrival, c.product_type, false, &dep.departure, dep.product_type, transfer_time, self.now);
            }
            if p > 0.0 {
                new_distribution.add_with(&dep_label.destination_arrival, p*remaining_probability, self.mean_only);
                remaining_probability = (1.0-p)*remaining_probability;
                if remaining_probability <= self.epsilon_feasible {
                    break;
                }
            }
        }
        new_distribution.feasible_probability = (1.0-remaining_probability).clamp(0.0, 1.0);
        if new_distribution.feasible_probability < 1.0 {
            new_distribution.normalize_with(self.mean_only, self.epsilon_feasible*self.epsilon_feasible);
        }
    }

    pub fn relevant_stations(&mut self, origin_idx: usize, destination_idx: usize, station_labels: &[Vec<ConnectionLabel>]) -> HashMap<usize, f32> {
        println!("from {} {} to {} {}", origin_idx, self.stations[origin_idx].name, destination_idx, self.stations[destination_idx].name);
        let mut stack = vec![(0, 1.0)];
        let mut initial = true;
        let mut weights_by_station_idx: HashMap<usize, f32> = HashMap::new();
        'outer: while !stack.is_empty() {
            let conn_with_prob = stack.pop().unwrap();
            let c = &self.connections[conn_with_prob.0];
            let station_idx = if initial { origin_idx } else { c.to_idx };

            if let Some(contr) = self.contraction {
                if !initial && contr.stop_to_group[c.to_idx] == contr.stop_to_group[origin_idx] {
                    continue;
                }
            } 

            let footpaths = &self.stations[station_idx].footpaths;
            if station_idx == destination_idx {
                *weights_by_station_idx.entry(station_idx).or_default() += conn_with_prob.1;
                for i in 0..footpaths.len() {
                    *weights_by_station_idx.entry(footpaths[i].target_location_idx).or_default() += conn_with_prob.1;
                }
                continue;
            }

            let mut departures = vec![&station_labels[station_idx]];
            let mut transfer_times = vec![self.stations[station_idx].transfer_time as i32];
            for i in 0..footpaths.len() {
                let stop_idx = footpaths[i].target_location_idx;
                if stop_idx == destination_idx {
                    *weights_by_station_idx.entry(station_idx).or_default() += conn_with_prob.1;
                    for i in 0..footpaths.len() {
                        *weights_by_station_idx.entry(footpaths[i].target_location_idx).or_default() += conn_with_prob.1;
                    }
                    continue 'outer;
                }
                let transfer_time = footpaths[i].duration as i32;
                departures.push(&station_labels[stop_idx]);
                transfer_times.push(transfer_time);
            }
            let mut is = vec![0; transfer_times.len()];
            let mut remaining_probability = 1.0;
            let mut last_departure: Option<&connection::StopInfo> = None;
            let mut last_product_type: i16 = 0;
        
            while remaining_probability > self.epsilon_feasible {
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

                /*if !self.domination && initial && last_departure.is_some() && dep.departure.projected() < last_departure.unwrap().projected() { 
                    continue;
                }*/
                if !initial && self.cut.contains(&(c.id, dep.id)) {
                    continue;
                }
                if !initial && !c.is_consecutive(dep) {
                    let mut transfer_time = transfer_times[min_k];
                    if let Some(contr) = self.contraction {
                        transfer_time = contr.get_transfer_time(c.to_idx, dep.from_idx) as i32;
                    }
                    p *= self.store.borrow_mut().before_probability(&c.arrival, c.product_type, false, &dep.departure, dep.product_type, transfer_time, self.now);
                }
                if !self.domination && last_departure.is_some() {
                    p *= self.store.borrow_mut().before_probability(last_departure.unwrap(), last_product_type, true, &dep.departure, dep.product_type, 1, self.now);
                }
                if p > 0.0 {
                    last_departure = Some(&dep.departure);
                    last_product_type = dep.product_type;
                }
                if p <= self.epsilon_reachable {
                    continue;
                }
                let dep_prob = p*remaining_probability*conn_with_prob.1/dep_label.destination_arrival.feasible_probability;
                if initial || !c.is_consecutive(dep) {
                    *weights_by_station_idx.entry(dep.from_idx).or_default() += dep_prob;
                    if station_idx != dep.from_idx {
                        *weights_by_station_idx.entry(station_idx).or_default() += dep_prob;
                    }
                }
                if !initial {
                    remaining_probability = (1.0-p).clamp(0.0,1.0)*remaining_probability;
                }
                if dep_prob > self.epsilon_feasible && dep_label.destination_arrival.feasible_probability >= 1.0-self.epsilon_feasible {
                    stack.push((dep_label.connection_idx, dep_prob));
                }
            }
            initial = false;
            
        }
        for w in &weights_by_station_idx.iter().map(|w| (*w.0, *w.1)).collect::<Vec<(usize, f32)>>() {
            for f in &self.stations[w.0].footpaths {
                *weights_by_station_idx.entry(f.target_location_idx).or_default() += w.1;
            }
        }
        println!("relevant stations: {}", weights_by_station_idx.len());
        weights_by_station_idx
    }

    pub fn relevant_connection_pairs(&mut self, weights_by_station_idx: &HashMap<usize, f32>) -> HashMap<i32, i32> {
        let mut stations: Vec<(&usize, &f32)> = weights_by_station_idx.iter().collect();
        stations.sort_unstable_by(|a,b| b.1.partial_cmp(a.1).unwrap());
        //println!("{:?}", stations.iter().take(500).map(|s| (&self.stations[s.0].name as &str, s.1)).collect::<Vec<(&str, f32)>>());
        let mut trip_id_to_conn_idxs: HashMap<i32, Vec<(usize, bool)>> = HashMap::new();
        for i in 0..std::cmp::min(stations.len(), 1000) {
            for arr in &self.stations[*stations[i].0].arrivals {
                self.insert_relevant_conn_idx(arr, &mut trip_id_to_conn_idxs, false);
            }
            for dep in &self.stations[*stations[i].0].departures {
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
                connection_pairs.insert(self.connections[trip[i+1].0].id as i32, self.connections[trip[i].0].id as i32);
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
        let mut cut = FxHashSet::default();
        prepare_and_query(&mut store, &mut connections, &stations, &mut cut, 0, 0, 0, 0, 0, 0.0, false);
        assert_eq!(cut.len(), 0);
    }
}