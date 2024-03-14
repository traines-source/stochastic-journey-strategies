use std::cell::RefCell;
use std::cmp::Ordering;
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

pub fn new<'a>(store: &'a mut distribution_store::Store, connections: &'a mut Vec<connection::Connection>, stations: &'a [connection::Station], cut: &'a mut FxHashSet<(usize, usize)>, order: &'a mut Vec<usize>, now: types::Mtime, epsilon_reachable: types::MFloat, epsilon_feasible: types::MFloat, mean_only: bool, domination: bool) -> Environment<'a> {
    if order.is_empty() {
        order.extend(0..connections.len());
    }
    Environment {
        store: RefCell::new(store),
        connections: connections,
        stations: stations,
        now,
        order,
        contraction: None,
        number_of_trips: 0,
        connection_pairs_reverse: vec![],
        max_dc: 90
    }
}

pub fn prepare<'a>(store: &'a mut distribution_store::Store, connections: &'a mut Vec<connection::Connection>, stations: &'a [connection::Station], cut: &'a mut FxHashSet<(usize, usize)>, order: &'a mut Vec<usize>, now: types::Mtime, epsilon: types::MFloat, mean_only: bool) -> Environment<'a> {
    let mut e = new(store, connections, stations, cut, order, now, epsilon, epsilon, mean_only, false);    
    println!("Starting topocsa...");
    e.preprocess();
    e
}

pub fn prepare_and_query<'a>(store: &'a mut distribution_store::Store, connections: &'a mut Vec<connection::Connection>, stations: &'a [connection::Station], cut: &'a mut FxHashSet<(usize, usize)>, origin: usize, destination: usize, start_time: types::Mtime, max_time: types::Mtime, now: types::Mtime, epsilon: types::MFloat, mean_only: bool) {
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
    order: &'a mut Vec<usize>,
    contraction: Option<&'a StationContraction>,
    number_of_trips: usize,
    connection_pairs_reverse: Vec<usize>,
    max_dc: types::Mtime
}

#[derive(Debug, Clone)]
pub struct ConnectionLabel {
    pub connection_idx: usize,
    pub destination_arrival: distribution::Distribution,
    pub prob_after: types::MFloat,
    pub departure_mean: types::MFloat
}

impl PartialEq for ConnectionLabel {
    fn eq(&self, other: &Self) -> bool {
        other.connection_idx == self.connection_idx
    }
}

impl Eq for ConnectionLabel {
}

impl Ord for ConnectionLabel {
    fn cmp(&self, other: &Self) -> Ordering {
        other.partial_cmp(&self).unwrap()
    }
}

impl PartialOrd for ConnectionLabel {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        other.departure_mean.partial_cmp(&self.departure_mean)
    }
}

impl<'a> Environment<'a> {

    pub fn set_station_contraction(&mut self, contr: &'a StationContraction) {
        self.contraction = Some(contr);
    }    
    
    pub fn preprocess(&mut self) {
        println!("Start preprocessing...");
        self.connections.sort_unstable_by(|a, b|
            b.departure.projected().cmp(&a.departure.projected())
        );
        for c in self.connections.iter().enumerate() {
            self.order[c.1.id] = c.0;
            if self.number_of_trips <= c.1.trip_id as usize {
                self.number_of_trips = (c.1.trip_id+1) as usize;
            }
        }
        println!("Done preprocessing.");
    }

    pub fn update(&mut self, connection_id: usize, is_departure: bool, location_idx: Option<usize>, in_out_allowed: Option<bool>, delay: Option<i16>) {
        let c = &mut self.connections[self.order[connection_id]];
        if location_idx.is_some() {
            if is_departure {
                c.from_idx = location_idx.unwrap();
            } else {
                c.to_idx = location_idx.unwrap();
            }
        }
        if in_out_allowed.is_some() {
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

    fn dominates(&self, q: &ConnectionLabel, p: &ConnectionLabel) -> bool {
        if q.destination_arrival.mean < p.destination_arrival.mean {
            return true;
        }
        if q.destination_arrival.mean == p.destination_arrival.mean && q.departure_mean > p.departure_mean {
            return true;
        }
        return false;
    }

    pub fn query(&mut self, _origin: usize, destination: usize, start_time: types::Mtime, max_time: types::Mtime) -> Vec<Vec<ConnectionLabel>> {
        let contr = self.contraction.unwrap();
        self.connection_pairs_reverse = vec![0; self.connections.len()];
        let mut station_labels: Vec<Vec<ConnectionLabel>> = (0..self.stations.len()).map(|i| Vec::new()).collect();
        let mut trip_labels: Vec<(types::MFloat, usize)> = vec![(types::MFloat::MAX, 0); self.number_of_trips]; 
        let max_delay = self.store.borrow().max_delay as types::Mtime;
        for i in 0..self.connections.len() {
            let c = &self.connections[i];

            if c.departure.projected()+max_delay < start_time {
                break;
            }
            if c.departure.projected() >= max_time {
                continue;
            }
            let stop_idx = contr.stop_to_group[c.to_idx];
            let dest_contr = contr.stop_to_group[destination];
            let mut t1 = types::MFloat::MAX;
            if stop_idx == dest_contr && c.arrival.in_out_allowed {
                let mut new_distribution = self.store.borrow().delay_distribution(&c.arrival, false, c.product_type, self.now);
                if c.to_idx != destination {
                    let contr = self.contraction.unwrap();
                    new_distribution = new_distribution.shift(contr.get_transfer_time(c.to_idx, destination) as i32);
                }
                t1 = new_distribution.mean;
            }
            let t2 = trip_labels[c.trip_id as usize].0;
            let mut t3 = types::MFloat::MAX;
            let mut cum = 0.0;
            let mut last_latest_arrival = -1;
            let mut mass = 0.0;
            for dep_label in station_labels[stop_idx].iter().rev() {
                if (dep_label.departure_mean as i32) < c.arrival.projected() {
                    continue;
                }
                if dep_label.destination_arrival.mean == types::MFloat::MAX {
                    break;
                }
                if !c.arrival.in_out_allowed {
                    break;
                }
                let transfer_time = 1;//contr.get_transfer_time(c.to_idx, self.connections[dep.connection_idx].from_idx) as i32;
                let latest_arrival = dep_label.departure_mean as i32 - c.arrival.projected() - transfer_time;
                
                let m = self.store.borrow_mut().between_probability_conn(c, last_latest_arrival+1, latest_arrival+1, self.now);
                cum += dep_label.destination_arrival.mean * m;
                mass += m;
                last_latest_arrival = latest_arrival;
                if dep_label.departure_mean as i32 > c.arrival.projected() + self.max_dc {
                    t3 = cum;
                    break;
                }
            }
            if t3 != types::MFloat::MAX {
                assert_float_absolute_eq!(mass, 1.0);
            }            
            let tc = t1.min(t2).min(t3);            
            if tc != types::MFloat::MAX {
                if tc < trip_labels[c.trip_id as usize].0 {
                    trip_labels[c.trip_id as usize] = (tc, i);
                    self.connection_pairs_reverse[i] = i;
                } else {
                    self.connection_pairs_reverse[i] = trip_labels[c.trip_id as usize].1;
                }
                if !c.departure.in_out_allowed {
                    continue;
                }
                let departure_station_idx = contr.stop_to_group[c.from_idx];
                let departures = station_labels.get_mut(departure_station_idx).unwrap();
                let q = &departures.last();
                let mut distr = distribution::Distribution::empty(0);
                distr.mean = tc;
                let mut p = ConnectionLabel{
                    connection_idx: i,
                    destination_arrival: distr,
                    prob_after: 1.0,
                    departure_mean: c.departure.projected() as types::MFloat
                };
                if let Some(q) = q {
                    if !self.dominates(q, &p) {
                        if q.departure_mean != p.departure_mean {
                            departures.push(p);
                        } else {
                            departures.last_mut().replace(&mut p);
                        }
                    }
                } else {
                    departures.push(p);
                }
            }
        }
        self.store.borrow().print_stats();
        station_labels
    }

    pub fn get_decision_graph(&self, origin: usize, destination: usize, station_labels: &Vec<Vec<ConnectionLabel>>) -> (Vec<Vec<ConnectionLabel>>, HashMap<i32, i32>) {
        let contr = self.contraction.unwrap();
        let mut connection_pairs: HashMap<i32, i32>  = HashMap::new();
        let mut decision_graph: Vec<Vec<ConnectionLabel>> = (0..self.stations.len()).map(|i| Vec::new()).collect();
        if station_labels[contr.stop_to_group[origin]].is_empty() {
            return (decision_graph, connection_pairs);
        }
        let mut priority_queue = std::collections::BinaryHeap::new();
        let origin_contr = contr.stop_to_group[origin];
        let p = station_labels[origin_contr].last().unwrap();
        priority_queue.push(p);

        while !priority_queue.is_empty() {
            let p = priority_queue.pop().unwrap();
            let c = &self.connections[p.connection_idx];
            let arr = &self.connections[self.connection_pairs_reverse[p.connection_idx] as usize];
            let stop_idx = contr.stop_to_group[arr.to_idx];
            let dest_contr = contr.stop_to_group[destination];
            
            connection_pairs.insert(arr.id as i32, c.id as i32);
            let existing_deps = decision_graph.get_mut(contr.stop_to_group[c.from_idx]).unwrap();
            if !existing_deps.last().is_some_and(|l| p.connection_idx == l.connection_idx) {
                existing_deps.push(p.clone());

                if dest_contr != stop_idx {
                    for next_p in station_labels[stop_idx].iter().rev() {
                        if next_p.departure_mean as i32 >= arr.arrival.projected() && next_p.departure_mean != f32::MAX {
                            priority_queue.push(next_p);
                        }
                        if next_p.departure_mean as i32 > arr.arrival.projected() + self.max_dc {
                            break;
                        }
                    }
                }
            }
        }
        (decision_graph, connection_pairs)
    }
}