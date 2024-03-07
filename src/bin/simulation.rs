use std::borrow::Borrow;
use std::collections::HashSet;
use std::env;
use std::error::Error;
use glob::glob;
use motis_nigiri::Timetable;
use ndarray_stats::interpolate::Higher;
use ndarray_stats::interpolate::Lower;
use ndarray_stats::interpolate::Nearest;
use ndarray_stats::Quantile1dExt;
use rustc_hash::FxHashSet;
use ndarray_stats::QuantileExt;
use ndarray_stats::interpolate::Linear;
use serde::Deserialize;
use serde::Serialize;
use stost::connection;
use stost::gtfs::GtfsTimetable;
use stost::gtfs::OriginDestinationSample;
use stost::gtfs::StationContraction;
use stost::types;
use std::collections::HashMap;
use std::io::Write;
use std::fs;
use std::time::Instant;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;
use stost::distribution_store;
use stost::gtfs;
use stost::distribution;
use stost::query::topocsa;
use ndarray;
use noisy_float::types::{n64, N64};

#[derive(Serialize, Deserialize, Debug, Clone)]
struct SimulationConfig {
    distributions_path: String,
    gtfs_path: String,
    gtfs_cache_path: String,
    gtfsrt_glob: String,
    samples_config_path: String,
    det_simulation: String,
    stoch_simulation: String,
    transfer: String,
	#[serde(default)]
    epsilon_reachable: f32,
	#[serde(default)]
    epsilon_feasible: f32,
	#[serde(default)]
    transfer_strategy: String,
    samples: usize,
	#[serde(default)]
    start_mams: Vec<i32>,
	#[serde(default)]
    start_date: Vec<i32>,
	#[serde(default)]
    num_days: i32,
    #[serde(default)]
    query_window: i32
}

fn load_config(path: &str) -> SimulationConfig {
    let buf = std::fs::read(path).unwrap();
    serde_json::from_slice(&buf).unwrap()
}

fn load_samples(path: &str) -> Vec<OriginDestinationSample> {
    let buf = std::fs::read(path).unwrap();
    serde_json::from_slice(&buf).unwrap()
}

fn load_simulation_run(path: &str) -> SimulationRun {
    let buf = std::fs::read(path).unwrap();
    serde_json::from_slice(&buf).unwrap()
}

fn day(year: i32, month: i32, day: i32) -> chrono::NaiveDate {
    chrono::NaiveDate::from_ymd_opt(year, month as u32, day as u32).unwrap()
}

fn resolve_connection_idx(
    journey: &motis_nigiri::Journey,
    leg_idx: usize,
    to: bool,
    mapping: &HashMap<(usize, u16), usize>,
    order: &[usize]
) -> usize {
    let leg = &journey.legs[leg_idx];
    let connid = mapping[&(leg.transport_idx, leg.day_idx)]
        + if to {
            leg.to_stop_idx - 1
        } else {
            leg.from_stop_idx
        } as usize;
    order[connid]
}

struct LogEntry {
    conn_id: usize,
    proj_dest_arr: f32,
    arrival_time_lower_bound: i32
}
struct Alternative {
    from_conn_idx: usize,
    to_conn_idx: usize,
    proj_dest_arr: f32
}

#[derive(Serialize, Deserialize, Debug)]
struct SimulationResult {
    departure: i32,
    original_dest_arrival_prediction: f32,
    actual_dest_arrival: Option<i32>,
    broken: bool,
	#[serde(default)]
    preprocessing_elapsed_ms: u128,
    algo_elapsed_ms: Vec<u128>,
    connections_taken: Vec<connection::Connection>,
    connection_missed: Option<connection::Connection>
}

impl SimulationResult {
    fn is_completed(&self) -> bool {
        self.broken || self.actual_dest_arrival.is_some()
    }
}

#[derive(Serialize, Deserialize, Debug)]
struct SimulationJourney {
    pair: (usize, usize, i32),
    start_time: u64,
    from_station: String,
    from_station_name: String,
    to_station: String,
    to_station_name: String,    
    det: SimulationResult,
    stoch: SimulationResult,
}

impl SimulationJourney {
    fn is_completed(&self) -> bool {
        self.det.is_completed() && self.stoch.is_completed()
    }
    fn is_broken(&self) -> bool {
        self.det.broken || self.stoch.broken
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct SimulationRun {
    simulation_run_at: u64,
    comment: String,
    config: SimulationConfig,
    results: Vec<SimulationJourney>
}

struct StochActions {
    station_labels: Vec<Vec<topocsa::ConnectionLabel>>,
    connection_pairs: HashMap<i32, i32>,
    connection_pairs_reverse: HashMap<usize, usize>,
    relevant_stations: HashMap<usize, f32>
}

struct Simulation {
    conf: SimulationConfig,
    store: distribution_store::Store,
    contr: Option<StationContraction>,
    det_actions: HashMap<(usize, usize, i32), motis_nigiri::Journey>,
    stoch_actions: HashMap<(usize, usize, i32), StochActions>,
    det_log: HashMap<(usize, usize, i32), Vec<LogEntry>>,
    stoch_log: HashMap<(usize, usize, i32), Vec<LogEntry>>,
    results: HashMap<(usize, usize, i32), SimulationJourney>,
}

impl Simulation {
    fn new(config_file: &str) -> Simulation {
        let conf = load_config(config_file);
        let mut store = distribution_store::Store::new();
        store.load_distributions(&conf.distributions_path);
        Simulation {
            conf: conf,
            store: store,
            contr: None,
            det_actions: HashMap::new(),
            stoch_actions: HashMap::new(),
            det_log: HashMap::new(),
            stoch_log: HashMap::new(),
            results: HashMap::new(),
        }
    }

    fn run_simulation(&mut self) -> Result<i32, Box<dyn std::error::Error>> {
        let mut t = None;
        let mut tt = gtfs::GtfsTimetable::new();

        let mut reference_ts = 0;
        let reference_offset = 5*1440;
        let mut next_start_mam_idx = 0;
        let mut day_idx = 0;
        
        let mut stop_pairs: Vec<(usize, usize, i32)> = vec![];
        
        let simulation_run_at = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();

        assert!(self.conf.num_days == 1 || self.conf.start_mams.last().unwrap()+self.conf.query_window-1440 < self.conf.start_mams[0], "last start_mam may not overlap with first start_mam of the next day");

        for f in glob(&self.conf.gtfsrt_glob).expect("Failed to read glob pattern") {
            let path = f.as_ref().unwrap().to_str().unwrap().to_owned();
            let mtime = Self::get_mtime_unix(f)?;
            self.reload_gtfs_if_necessary(&mut reference_ts, &mut next_start_mam_idx, mtime, reference_offset, &mut stop_pairs, day_idx, &mut t, &mut tt);
            let current_time = Self::get_current_time(mtime, reference_ts); 
            if current_time < self.conf.start_mams[0]+reference_offset {
                continue;
            }          
            self.load_gtfsrt(&mut tt, current_time, path, &t);
            let mut timing_preprocessing = 0;
            let mut do_continue = false;
            for pair in &stop_pairs {
                println!("Pair: {:?}", pair);
                self.initialize_if_necessary(pair, &mut tt, current_time, &mut timing_preprocessing, &t, reference_ts);
                if self.det_actions.get(pair).is_none() || pair.2 + self.conf.query_window < current_time {
                    continue;
                }
                self.update_if_necessary(pair, &mut tt, &t, current_time, &mut timing_preprocessing);
                println!("stoch...");
                let mut repeat = true;
                while repeat {
                    repeat = false;
                    let current_stop_idx = Self::get_current_stop_idx(current_time, *pair, self.stoch_log.get_mut(pair).unwrap(), &mut self.results.get_mut(pair).unwrap().stoch, &tt);
                    if current_stop_idx.is_some() {
                        let alternatives = Self::get_stoch_alternatives(current_stop_idx.unwrap(), &tt, &self.stoch_actions[pair], &self.contr);
                        repeat = Self::step(current_time, pair.2, current_stop_idx.unwrap(), &alternatives, self.stoch_log.get_mut(pair).unwrap(), &mut self.results.get_mut(pair).unwrap().stoch, &tt);
                    }
                }
                println!("det...");
                repeat = true;
                while repeat {
                    repeat = false;
                    let current_stop_idx = Self::get_current_stop_idx(current_time, *pair, self.det_log.get_mut(pair).unwrap(), &mut self.results.get_mut(pair).unwrap().det, &tt);
                    if current_stop_idx.is_some() {
                        let alternatives = Self::get_det_alternatives(current_stop_idx.unwrap(), &tt, &self.det_actions[pair]);
                        repeat = Self::step(current_time, pair.2, current_stop_idx.unwrap(), &alternatives, self.det_log.get_mut(pair).unwrap(), &mut self.results.get_mut(pair).unwrap().det, &tt);
                        let cid = self.det_log[pair].last();
                        if cid.is_some() && !self.stoch_actions[pair].connection_pairs.is_empty() && !self.stoch_actions[pair].connection_pairs.contains_key(&(cid.unwrap().conn_id as i32)) && !self.stoch_actions[pair].connection_pairs_reverse.contains_key(&cid.unwrap().conn_id) {
                            println!("WARN: connection from det not contained in connection pairs {} {} {}", cid.unwrap().conn_id, tt.connections[tt.order[cid.unwrap().conn_id]].from_idx, tt.connections[tt.order[cid.unwrap().conn_id]].to_idx);
                        }
                    }
                }
                self.clear_stoch_actions_if_necessary(*pair);
                if !self.results[pair].is_completed() {
                    do_continue = true;
                }
            }
            if next_start_mam_idx == self.conf.start_mams.len() && (!do_continue || current_time-1440-reference_offset >= self.conf.start_mams[0]) {
                println!("All simulations completed ({}) for the day. Stopping at current_time {}.", !do_continue, current_time);
                self.write_results(simulation_run_at, day_idx);
                
                stop_pairs.clear();
                self.det_actions.clear();
                self.stoch_actions.clear();
                self.det_log.clear();
                self.stoch_log.clear();

                day_idx += 1;
                next_start_mam_idx = 0;
                if day_idx >= self.conf.num_days {
                    break;
                }
            }
        }
        if !stop_pairs.is_empty() {
            println!("Reached end of GTFSRT without having completed everything.");
            self.write_results(simulation_run_at, day_idx);            
        }
        Ok(0)
    }

    fn initialize_if_necessary(&mut self, pair: &(usize, usize, i32), tt: &mut GtfsTimetable, current_time: i32, timing_preprocessing: &mut u128, t: &Option<Timetable>, reference_ts: u64) {
        if self.results.get(&pair).is_none() {
            let mut env = Self::new_env(&mut self.store, &mut tt.connections, &tt.stations, &mut tt.cut, &mut tt.order, &self.contr, &self.conf, current_time, true);
            Self::preprocess_if_necessary(&mut env, timing_preprocessing);
            let start = Instant::now();
            let stoch = env.query(pair.0, pair.1, pair.2, pair.2+self.conf.query_window);
            let mut timing_stoch = start.elapsed().as_millis();
            let (min_journey, timing_det) = get_min_det_journey(t, pair.0, pair.1, pair.2);
            let stoch_exist_alternatives = stoch.get(self.contr.as_ref().map(|contr| contr.stop_to_group[pair.0]).unwrap_or(pair.0)).is_some_and(|s| s.len() > 0);
            if min_journey.is_none() || !stoch_exist_alternatives {
                println!("Infeasible for either det or stoch, skipping. det: {:?} stoch: {:?}", min_journey, stoch_exist_alternatives);
            } else {
                self.det_actions.insert(*pair, min_journey.unwrap());
                let relevant_stations = if self.conf.stoch_simulation.starts_with("adaptive_online_relevant") {
                    let start = Instant::now();
                    let mut relevant_stations = env.relevant_stations(pair.0, pair.1, &stoch);
                    timing_stoch += start.elapsed().as_millis();
                    println!("Enriching relevant stations...");
                    for l in &*self.det_actions[pair].legs {
                        println!("{} {} {:?}", l.from_location_idx, tt.stations[l.from_location_idx].name, relevant_stations.insert(l.from_location_idx, 1000.0));
                        println!("{} {} {:?}", l.to_location_idx, tt.stations[l.to_location_idx].name, relevant_stations.insert(l.to_location_idx, 1000.0));
                    }
                    relevant_stations
                } else {
                    HashMap::new()
                };
                self.stoch_actions.insert(*pair, StochActions{
                    station_labels: stoch,
                    connection_pairs_reverse: HashMap::new(),
                    connection_pairs: HashMap::new(),
                    relevant_stations: relevant_stations
                });
            }
            self.det_log.insert(*pair, vec![]);
            self.stoch_log.insert(*pair, vec![]);
            self.results.insert(*pair, SimulationJourney {
                pair: *pair,
                start_time: reference_ts+pair.2 as u64*60,
                from_station: tt.stations[pair.0].id.clone(),
                from_station_name: tt.stations[pair.0].name.clone(),
                to_station: tt.stations[pair.1].id.clone(),
                to_station_name: tt.stations[pair.1].name.clone(),
                det: SimulationResult {
                    departure: 0,
                    original_dest_arrival_prediction: 0.0,
                    actual_dest_arrival: None,
                    preprocessing_elapsed_ms: 0,
                    algo_elapsed_ms: vec![timing_det],
                    broken: false,
                    connections_taken: vec![],
                    connection_missed: None
                },
                stoch: SimulationResult {
                    departure: 0,
                    original_dest_arrival_prediction: 0.0,
                    actual_dest_arrival: None,
                    preprocessing_elapsed_ms: *timing_preprocessing,
                    algo_elapsed_ms: vec![timing_stoch],
                    broken: false,
                    connections_taken: vec![],
                    connection_missed: None                        
                }
            });
        }
    }

    fn update_if_necessary(&mut self, pair: &(usize, usize, i32), tt: &mut GtfsTimetable, t: &Option<Timetable>, current_time: i32, timing_preprocessing: &mut u128) {
        let det_arrival_time = Self::get_arrival_time(&self.det_log[&pair], pair.2, tt);
        if current_time >= det_arrival_time
            && self.results[&pair].det.actual_dest_arrival.is_none()
            && self.conf.det_simulation.starts_with("priori_online") {
            let stuck_at = self.det_log[pair].last().map(|l| tt.connections[tt.order[l.conn_id]].to_idx).unwrap_or(pair.0);
            let mut fixed_arrival_time = None;
            if self.results[pair].det.broken {
                println!("Replacing broken det itinerary.");
                fixed_arrival_time = Self::fix_if_sitting_in_cancelled_trip(self.det_log.get_mut(&pair).unwrap(), &self.results[&pair].det, pair.2, tt);
            }
            if self.results[pair].det.broken || self.conf.det_simulation == "priori_online" {
                let (min_journey, timing_det) = get_min_det_journey(t, stuck_at, pair.1, fixed_arrival_time.unwrap_or(current_time));
                if min_journey.is_some() {
                    self.det_actions.insert(*pair, min_journey.unwrap());
                    let r = self.results.get_mut(pair).unwrap();
                    r.det.broken = false;
                    r.det.algo_elapsed_ms.push(timing_det);
                }
            }
        }
        let stoch_arrival_time = Self::get_arrival_time(&self.stoch_log[&pair], pair.2, tt);
        if current_time >= stoch_arrival_time
            && self.results[&pair].stoch.actual_dest_arrival.is_none()
            && self.conf.stoch_simulation.starts_with("adaptive_online") {
            let mut fixed_arrival_time = None;
            let mut stuck_at = None;
            if self.results[pair].stoch.broken {
                fixed_arrival_time = Self::fix_if_sitting_in_cancelled_trip(self.stoch_log.get_mut(&pair).unwrap(), &self.results[&pair].stoch, pair.2, tt);
                stuck_at = Some(self.stoch_log[pair].last().map(|l| tt.connections[tt.order[l.conn_id]].to_idx).unwrap_or(pair.0));
            }
            let mut env = Self::new_env(&mut self.store, &mut tt.connections, &tt.stations, &mut tt.cut, &mut tt.order, &self.contr, &self.conf, fixed_arrival_time.unwrap_or(current_time), self.conf.stoch_simulation != "adaptive_online_relevant_with_distr");
            Self::preprocess_if_necessary(&mut env, timing_preprocessing);
            let start = Instant::now();
            self.stoch_actions.entry(*pair).and_modify(|a| {
                if let Some(sidx) = stuck_at {
                    a.relevant_stations.insert(sidx, 1000.0);
                    for f in &tt.stations[sidx].footpaths {
                        a.relevant_stations.insert(f.target_location_idx, 1000.0);
                    }
                }
                a.connection_pairs = env.relevant_connection_pairs(&a.relevant_stations);
                a.connection_pairs_reverse = a.connection_pairs.iter().map(|(arr,dep)| (*dep as usize, *arr as usize)).collect();
            });
            let stoch = env.pair_query(pair.0, pair.1, pair.2, pair.2+self.conf.query_window, &self.stoch_actions[pair].connection_pairs);
            let timing_stoch = start.elapsed().as_millis();
            println!("elapsed: {} connpairs: {}", timing_stoch, self.stoch_actions[pair].connection_pairs.len());
            self.stoch_actions.get_mut(pair).unwrap().station_labels = stoch;
            self.results.get_mut(&pair).unwrap().stoch.algo_elapsed_ms.push(timing_stoch);
        }
    }

    fn fix_if_sitting_in_cancelled_trip(log: &mut Vec<LogEntry>, result: &SimulationResult, start_time: i32, tt: &mut GtfsTimetable) -> Option<types::Mtime> {
        if log.len() > 0 {
            let mut c_id = log.last().unwrap().conn_id;
            let mut c = &tt.connections[tt.order[c_id]];
            if !c.arrival.in_out_allowed {
                let boarded_trip_at_id = result.connections_taken.get(result.connections_taken.len()-2).map(|r| r.id).unwrap_or(c_id);
                while !c.arrival.in_out_allowed && c_id > boarded_trip_at_id {
                    c_id -= 1;
                    c = &tt.connections[tt.order[c_id]];
                }
                if !c.arrival.in_out_allowed {
                    log.pop();
                    println!("Sitting in cancelled trip. Returning to stop where boarded, at {}. Now at {:?}", boarded_trip_at_id, log.last().map(|l| l.conn_id).unwrap_or(0));
                } else {
                    let last_log = log.last_mut().unwrap();
                    println!("Sitting in cancelled trip. Returning to last valid stop, boarded at {}, updating connid {} to {}. Now at {:?}", boarded_trip_at_id, last_log.conn_id, c_id, c);
                    last_log.conn_id = c_id;
                }
                return Some(Self::get_arrival_time(log, start_time, tt))
            }
        }
        None
    }

    fn load_gtfsrt(&mut self, tt: &mut GtfsTimetable, current_time: i32, path: String, t: &Option<Timetable>) {
        let mut env = Self::new_env(&mut self.store, &mut tt.connections, &tt.stations, &mut tt.cut, &mut tt.order, &self.contr, &self.conf, current_time, true);
        println!("Loading GTFSRT {}", path);
        gtfs::load_realtime(
            &path,
            t.as_ref().unwrap(),
            &tt.transport_and_day_to_connection_id,
            |connection_id: usize, is_departure: bool, location_idx: Option<usize>, in_out_allowed: Option<bool>, delay: Option<i16>| {
                env.update(connection_id, is_departure, location_idx, in_out_allowed, delay)
            },
        );
        gtfs::sort_station_departures_asc(&mut tt.stations, &tt.connections, &tt.order);
    }

    fn preprocess_if_necessary(env: &mut topocsa::Environment, timing_preprocessing: &mut u128) {
        if *timing_preprocessing != 0 {
            return;
        }
        let start = Instant::now();
        env.preprocess();
        *timing_preprocessing = start.elapsed().as_millis();
    }

    fn write_results(&mut self, simulation_run_at: u64, day_idx: i32) {
        let filename = format!("./simulation/runs/{}.{}.{}.{}.{}.ign.json", simulation_run_at, self.conf.det_simulation, self.conf.stoch_simulation, self.conf.transfer, day_idx);
        let run = SimulationRun {
            simulation_run_at: simulation_run_at,
            comment: "".to_string(),
            config: self.conf.clone(),
            results: std::mem::replace(&mut self.results, HashMap::new()).into_values().collect(),
        };
        let buf = serde_json::to_vec(&run).unwrap();
        let mut file = fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(filename).expect("file not openable");
        file.write_all(&buf).expect("error writing file");
        println!("Results written.");
    }
            
    fn reload_gtfs_if_necessary(&mut self, reference_ts: &mut u64, next_start_mam_idx: &mut usize, mtime: u64, reference_offset: i32, stop_pairs: &mut Vec<(usize, usize, i32)>, day_idx: i32, t: &mut Option<Timetable>, tt: &mut GtfsTimetable) {
        if *reference_ts == 0 || *next_start_mam_idx < self.conf.start_mams.len() && Self::get_current_time(mtime, *reference_ts) >= self.conf.start_mams[*next_start_mam_idx]+reference_offset {
            let next_start_mam = self.conf.start_mams[*next_start_mam_idx];
            println!("Beginning next start_mam {}", next_start_mam);
            stop_pairs.extend(load_samples(&self.conf.samples_config_path).iter().take(self.conf.samples).map(|s| (s.from_idx, s.to_idx, next_start_mam+reference_offset)));
            //let number_of_days = if next_start_mam+self.conf.query_window > 1440 { 2 } else { 1 };
            //let has_changed = number_of_days == 2 && self.conf.start_mams[*next_start_mam_idx-1]+self.conf.query_window <= 1440; TODO stable connids?
            let number_of_days = 2;
            if *next_start_mam_idx == 0 {
                println!("Loading GTFS day_idx {} days {}", day_idx, number_of_days);
                *t = Some(gtfs::load_timetable(&self.conf.gtfs_path, day(self.conf.start_date[0], self.conf.start_date[1], self.conf.start_date[2]+day_idx), day(self.conf.start_date[0], self.conf.start_date[1], self.conf.start_date[2]+day_idx+number_of_days)));
                *reference_ts = t.as_ref().unwrap().get_start_day_ts() as u64;
                *tt = gtfs::GtfsTimetable::new();
                let mut routes = vec![];
                tt.transport_and_day_to_connection_id = gtfs::retrieve(t.as_ref().unwrap(), &mut tt.stations, &mut routes, &mut tt.connections);
                self.contr = Some(gtfs::get_station_contraction(&tt.stations));
                if self.conf.transfer == "short" {
                    connection::shorten_footpaths(&mut tt.stations);
                }
            }
            *next_start_mam_idx += 1;
        }
    }

    fn get_mtime_unix(f: Result<std::path::PathBuf, glob::GlobError>) -> Result<u64, Box<dyn Error>> {
        Ok(fs::metadata(f?)?
            .modified()?
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs())
    }

    fn get_current_time(mtime: u64, reference_ts: u64) -> i32 {
        ((mtime-reference_ts)/60) as i32
    }

    fn new_env<'a>(store: &'a mut distribution_store::Store, connections: &'a mut Vec<connection::Connection>, stations: &'a Vec<connection::Station>, cut: &'a mut FxHashSet<(usize, usize)>, order: &'a mut Vec<usize>, contr: &'a Option<StationContraction>, conf: &SimulationConfig, now: types::Mtime, mean_only: bool) -> topocsa::Environment<'a> {
        let mut env = topocsa::new(
            store,
            connections,
            stations,
            cut,
            order,
            now,
            conf.epsilon_reachable,
            conf.epsilon_feasible,
            mean_only,
            conf.transfer_strategy == "domination"
        );
        if let Some(contraction) = contr {
            env.set_station_contraction(contraction)
        }
        env
    }

    fn get_current_stop_idx(current_time: i32, pair: (usize, usize, i32), log: &mut Vec<LogEntry>, result: &mut SimulationResult, tt: &GtfsTimetable) -> Option<usize> {
        if result.actual_dest_arrival.is_some() {
            println!("Completed.");
            None
        } else if log.len() == 0 {
            Some(pair.0)
        } else {
            let last_c = &tt.connections[tt.order[log.last().unwrap().conn_id]];
            let current_stop_idx = last_c.to_idx;
            let footpaths = &tt.stations[current_stop_idx].footpaths;
            let mut i = 0;
            while i <= footpaths.len() {
                let stop_idx = if i == footpaths.len() { current_stop_idx } else { footpaths[i].target_location_idx };
                if stop_idx == pair.1 {
                    if current_time >= last_c.arrival.projected() && last_c.arrival.in_out_allowed {
                        Self::update_connections_taken_from_last_log(result, log, tt);
                        let walking_time = if i == footpaths.len() { 0 } else { footpaths[i].duration as i32 };
                        result.actual_dest_arrival = Some(last_c.arrival.projected()+walking_time);
                    }
                    return None;
                }
                i += 1;
            }
            Some(current_stop_idx)
        }
    }

    fn get_det_alternatives(current_stop_idx: usize, tt: &GtfsTimetable, det_actions: &motis_nigiri::Journey) -> Vec<Alternative> {
        let mut next_leg = 0;
        for l in &*det_actions.legs {
            if l.from_location_idx == current_stop_idx {
                if l.is_footpath {
                    next_leg += 1;
                }
                break;
            }
            next_leg += 1;
        }
        if next_leg >= det_actions.legs.len() {
            println!("failed to find journey continuation (platform change?): {:?} current_stop: {} {:?}", det_actions, current_stop_idx, tt.stations[current_stop_idx]);
            return vec![];
        }
        let departure_idx = resolve_connection_idx(det_actions, next_leg, false, &tt.transport_and_day_to_connection_id, &tt.order);
        let arrival_idx = resolve_connection_idx(det_actions, next_leg, true, &tt.transport_and_day_to_connection_id, &tt.order);
        println!("current_stop: {} dep: {} {:?}", current_stop_idx, tt.connections[departure_idx].from_idx, tt.connections[departure_idx].departure);
        let alternatives = vec![Alternative{
            from_conn_idx: departure_idx,
            to_conn_idx: arrival_idx,
            proj_dest_arr: det_actions.dest_time as f32
        }];
        alternatives
    }

    fn get_stoch_alternatives(current_stop_idx: usize, tt: &GtfsTimetable, stoch_actions: &StochActions, contr: &Option<StationContraction>) -> Vec<Alternative> {
        let mut alternatives: Vec<Alternative> = vec![];
        if let Some(contr) = contr {
            let stop_idx = contr.stop_to_group[current_stop_idx];
            Self::extend_alternatives_by_station_labels(stoch_actions, stop_idx, &mut alternatives, tt);
        } else {
            let footpaths = &tt.stations[current_stop_idx].footpaths;
            for i in 0..footpaths.len()+1 {
                let stop_idx = if i == footpaths.len() { current_stop_idx } else { footpaths[i].target_location_idx };
                println!("label len: {} {} {} {} transftime: {}", current_stop_idx, stop_idx, stoch_actions.station_labels.iter().filter(|l| l.len() > 0).count(), stoch_actions.connection_pairs_reverse.len(), if i == footpaths.len() { 0 } else { footpaths[i].duration });
                Self::extend_alternatives_by_station_labels(stoch_actions, stop_idx, &mut alternatives, tt);
            }
        }
        alternatives.sort_unstable_by(|a, b| a.proj_dest_arr.partial_cmp(&b.proj_dest_arr).unwrap());
        alternatives
    }

    fn extend_alternatives_by_station_labels(stoch_actions: &StochActions, stop_idx: usize, alternatives: &mut Vec<Alternative>, tt: &GtfsTimetable) {
        let station_labels = stoch_actions.station_labels.get(stop_idx);
        if station_labels.is_none() {
            return;
        }
        alternatives.extend(station_labels.unwrap().iter().filter_map(|l| {
            if l.destination_arrival.mean == 0.0 {
                panic!("weirdly 0");
            }
            if l.destination_arrival.feasible_probability < 0.5 {
                return None // TODO properly use transfer strategy?
            }
            Some(Alternative{
                from_conn_idx: l.connection_idx,
                to_conn_idx: if stoch_actions.connection_pairs_reverse.is_empty() { l.connection_idx } else { tt.order[stoch_actions.connection_pairs_reverse[&tt.connections[l.connection_idx].id]] },
                proj_dest_arr: l.destination_arrival.mean
            })
        }));
    }

    fn clear_stoch_actions_if_necessary(&mut self, pair: (usize, usize, i32)) {
        if self.conf.stoch_simulation.starts_with("adaptive_online") {
            self.stoch_actions.entry(pair).and_modify(|a| {
                a.station_labels.clear();
                a.station_labels.shrink_to_fit();
                a.connection_pairs.clear();
                a.connection_pairs.shrink_to_fit();
                a.connection_pairs_reverse.clear();
                a.connection_pairs_reverse.shrink_to_fit();
            });
        }
    } 

    fn step(current_time: i32, start_time: i32, current_stop_idx: usize, alternatives: &[Alternative], log: &mut Vec<LogEntry>, result: &mut SimulationResult, tt: &GtfsTimetable) -> bool {
        if log.len() > 10000 || result.connections_taken.len() > 10000 {
            panic!("Log len exceeded");
        }
        let arrival_time = Self::get_arrival_time(log, start_time, tt);
        let mut alternatives_still_available = false;
        println!("current_time: {} current_stop_idx: {} arrival: {}", current_time, current_stop_idx, arrival_time);
        if current_time >= arrival_time {
            for alt in alternatives {
                let next_c = &tt.connections[alt.from_conn_idx];
                let transfer_time = Self::get_transfer_time(current_stop_idx, next_c.from_idx, log.is_empty(), &tt.stations); 
                if Self::can_take(next_c, arrival_time, transfer_time, log, tt) {    
                    if current_time >= next_c.departure.projected() { // TODO require not too long ago?
                        if log.len() > 0 {
                            Self::update_connections_taken_from_last_log(result, log, tt);
                        } else {
                            result.departure = next_c.departure.projected();
                            result.original_dest_arrival_prediction = alt.proj_dest_arr;
                        }
                        Self::update_connections_taken(result, &next_c, &tt.stations, alt.proj_dest_arr);
                        println!("step {} {} {} from/to: {} {} trip: {} arr: {} {} dep: {} {} to_conn: dp: {} {} arr: {} {} from/to: {} {} trip: {}", arrival_time, alt.from_conn_idx, alt.to_conn_idx, next_c.from_idx, next_c.to_idx, next_c.trip_id, next_c.arrival.scheduled, next_c.arrival.projected(), next_c.departure.scheduled, next_c.departure.projected(), tt.connections[alt.to_conn_idx].departure.scheduled, tt.connections[alt.to_conn_idx].departure.projected(), tt.connections[alt.to_conn_idx].arrival.scheduled, tt.connections[alt.to_conn_idx].arrival.projected(), tt.connections[alt.to_conn_idx].from_idx, tt.connections[alt.to_conn_idx].to_idx, tt.connections[alt.to_conn_idx].trip_id);
                        log.push(LogEntry{
                            conn_id: tt.connections[alt.to_conn_idx].id,
                            proj_dest_arr: alt.proj_dest_arr,
                            arrival_time_lower_bound: std::cmp::max(next_c.departure.projected(), arrival_time)
                        });
                        return true;
                    }
                    alternatives_still_available = true;
                    break;
                }
            }
            if !alternatives_still_available && !result.broken {
                if log.len() > 0 {
                    Self::update_connections_taken_from_last_log(result, log, tt);
                }
                if alternatives.len() > 0 {
                    result.connection_missed = Some(tt.connections[alternatives.last().unwrap().from_conn_idx].clone());
                }
                println!("Setting to broken.");
                result.broken = true;
            }
        }
        return false;
    }

    fn get_arrival_time(log: &Vec<LogEntry>, start_time: i32, tt: &GtfsTimetable) -> i32 {
        if log.len() == 0 {
            start_time
        } else {
            let c = &tt.connections[tt.order[log.last().unwrap().conn_id]];
            std::cmp::max(std::cmp::max(c.arrival.projected(), c.departure.projected()), log.last().unwrap().arrival_time_lower_bound) // TODO enforce while gtfsrt updating?
        }
    }

    fn can_take(next_c: &connection::Connection, arrival_time: i32, transfer_time: i32, log: &mut Vec<LogEntry>, tt: &GtfsTimetable) -> bool {
        if log.len() > 0 {
            let c = &tt.connections[tt.order[log.last().unwrap().conn_id]];
            if c.is_consecutive(next_c) {
                return true;
            }
            if !c.arrival.in_out_allowed {
                return false;
            }
        }
        if !next_c.departure.in_out_allowed {
            return false;
        }
        next_c.departure.projected() >= arrival_time+transfer_time
    }

    fn update_connections_taken_from_last_log(result: &mut SimulationResult, log: &[LogEntry], tt: &GtfsTimetable) {
        Self::update_connections_taken(result, &tt.connections[tt.order[log.last().unwrap().conn_id]], &tt.stations, log.last().unwrap().proj_dest_arr);
    }

    fn update_connections_taken(result: &mut SimulationResult, connection: &connection::Connection, stations: &[connection::Station], proj_dest_arr: f32) {
        let mut conn = connection.clone();
        conn.destination_arrival.replace(Some(distribution::Distribution {
            feasible_probability: 1.0,
            histogram: vec![],
            start: 0,
            mean: proj_dest_arr
        }));
        conn.message = format!("from: {} {} to: {} {}", stations[conn.from_idx].id, stations[conn.from_idx].name, stations[conn.to_idx].id, stations[conn.to_idx].name);
        result.connections_taken.push(conn);
        if result.broken {
            println!("Updating broken journey to make it feasible again.");
        }
        result.broken = false;
    }

    fn get_transfer_time(from_stop_idx: usize, to_stop_idx: usize, is_first: bool, stations: &[connection::Station]) -> i32 {
        if from_stop_idx == to_stop_idx {
            if is_first {
                return 0;
            }
            return stations[from_stop_idx].transfer_time as i32;
        }
        for f in &stations[from_stop_idx].footpaths {
            if f.target_location_idx == to_stop_idx {
                return std::cmp::max(f.duration, 1) as i32;
            }
        }
        panic!("Tried walking where no walking connection exists from {} to {}", from_stop_idx, to_stop_idx);
    }
}

fn get_min_det_journey(t: &Option<Timetable>, origin_idx: usize, destination_idx: usize, current_time: i32) -> (Option<motis_nigiri::Journey>, u128) {
    let start = Instant::now();
    let pareto = t.as_ref().unwrap().get_journeys(origin_idx, destination_idx, current_time, false);
    let timing_det = start.elapsed().as_millis();
    let min_journey = pareto.journeys.into_iter().min_by_key(|j| j.dest_time);
    (min_journey, timing_det)
}

struct SimulationAnalysis {
    baseline_infeasible: i32,
    baseline_broken: i32,
    target_infeasible: i32,
    target_broken: i32,
    baseline_and_target_infeasible: i32,
    delta_baseline_predicted_actual: Vec<f32>,
    delta_target_predicted_actual: Vec<f32>,
    delta_baseline_target_predicted: Vec<f32>,
    delta_baseline_predicted_target_actual: Vec<f32>,
    delta_baseline_target_actual_arrival: Vec<f32>,
    delta_baseline_target_actual_travel_time: Vec<f32>,
    target_actual_travel_time: Vec<f32>,
    baseline_algo_elapsed: Vec<f32>,
    target_preprocessing_elapsed: Vec<f32>,
    target_first_algo_elapsed: Vec<f32>,
    target_algo_elapsed: Vec<f32>
}

pub fn analyze_simulation(run_file: &str, baseline_file: Option<&str>) {
    let run = load_simulation_run(run_file);
    println!("\nComparison between stoch target and det target");
    analyze_run(run.results.iter().map(|r| &r.det).collect(), run.results.iter().map(|r| &r.stoch).collect(), run.results.iter().collect());
    //analyze_run(run.results.iter().filter(|r| r.pair.2 < 8000).map(|r| &r.det).collect(), run.results.iter().filter(|r| r.pair.2 < 8000).map(|r| &r.stoch).collect(), run.results.iter().filter(|r| r.pair.2 < 8000).collect());
    if let Some(file) = baseline_file {
        let baseline = load_simulation_run(file);
        let baseline_map = HashMap::from_iter(baseline.results.iter().map(|r| ((r.pair.0, r.pair.1, if r.pair.2 < 7700 {7620} else {r.pair.2}), r))); // TODO remove hack
        println!("\nComparison between stoch target and det baseline");
        analyze_run_with_separate_baseline(&baseline_map, run.results.iter().collect(), false);
        println!("\nComparison between stoch target and stoch baseline");
        analyze_run_with_separate_baseline(&baseline_map, run.results.iter().collect(), true);
    }
}

fn analyze_run_with_separate_baseline(baseline: &HashMap<(usize, usize, i32), &SimulationJourney>, target: Vec<&SimulationJourney>, both_stoch: bool) {
    let mut baseline_list = vec![];
    let mut target_list = vec![];
    for target_journey in &target {
        if let Some(baseline_journey) = baseline.get(&target_journey.pair) {
            baseline_list.push(if both_stoch { &baseline_journey.stoch } else { &baseline_journey.det });
            target_list.push(&target_journey.stoch);
        } 
    }
    analyze_run(baseline_list, target_list, target);
}

fn analyze_run(baseline: Vec<&SimulationResult>, target: Vec<&SimulationResult>, meta: Vec<&SimulationJourney>) {
    let mut a = SimulationAnalysis {
        baseline_infeasible: 0,
        baseline_broken: 0,
        target_infeasible: 0,
        target_broken: 0,
        baseline_and_target_infeasible: 0,
        delta_baseline_predicted_actual: vec![],
        delta_target_predicted_actual: vec![],
        delta_baseline_target_predicted: vec![],
        delta_baseline_predicted_target_actual: vec![],
        delta_baseline_target_actual_arrival: vec![],
        delta_baseline_target_actual_travel_time: vec![],
        target_actual_travel_time: vec![],
        baseline_algo_elapsed: vec![],
        target_preprocessing_elapsed: vec![],
        target_first_algo_elapsed: vec![],
        target_algo_elapsed: vec![]
    };
    assert_eq!(baseline.len(), target.len());
    for i in 0..baseline.len() {
        analyze_result(&mut a, baseline[i], target[i], &meta[i]);
    }
    println!("infeasible: both: {} baseline: {} target: {} broken: baseline: {} target: {} feasible: both: {} total: {}", a.baseline_and_target_infeasible, a.baseline_infeasible, a.target_infeasible, a.baseline_broken, a.target_broken, a.delta_baseline_target_actual_travel_time.len(), baseline.len());
    summary(a.delta_baseline_predicted_actual, "delta_baseline_predicted_actual");
    summary(a.delta_target_predicted_actual, "delta_target_predicted_actual");
    summary(a.delta_baseline_target_predicted, "delta_baseline_target_predicted");
    summary(a.delta_baseline_predicted_target_actual, "delta_baseline_predicted_target_actual");
    summary(a.delta_baseline_target_actual_arrival, "delta_baseline_target_actual_arrival");
    summary(a.delta_baseline_target_actual_travel_time, "delta_baseline_target_actual_travel_time");
    summary(a.target_actual_travel_time, "target_actual_travel_time");
    summary(a.baseline_algo_elapsed, "baseline_algo_elapsed");
    summary(a.target_first_algo_elapsed, "target_first_algo_elapsed");
    summary(a.target_algo_elapsed, "target_algo_elapsed");
    summary(a.target_preprocessing_elapsed, "target_preprocessing_elapsed");
}

fn analyze_result(a: &mut SimulationAnalysis, baseline: &SimulationResult, target: &SimulationResult, meta: &SimulationJourney) {
    if baseline.actual_dest_arrival.is_some() {
        a.delta_baseline_predicted_actual.push(baseline.actual_dest_arrival.unwrap() as f32-baseline.original_dest_arrival_prediction);
        a.baseline_algo_elapsed.extend(baseline.algo_elapsed_ms.iter().map(|e| *e as f32));
    } else {
        a.baseline_infeasible += 1;
        if baseline.broken {
            a.baseline_broken += 1;
        }
    }
    if target.actual_dest_arrival.is_some() {
        a.delta_target_predicted_actual.push(target.actual_dest_arrival.unwrap() as f32-target.original_dest_arrival_prediction);
        a.target_actual_travel_time.push((target.actual_dest_arrival.unwrap()-target.departure) as f32);
        a.target_first_algo_elapsed.push(target.algo_elapsed_ms[0] as f32);
        a.target_algo_elapsed.extend(target.algo_elapsed_ms.iter().skip(1).map(|e| *e as f32));
        a.target_preprocessing_elapsed.push(target.preprocessing_elapsed_ms as f32);
    } else {
        a.target_infeasible += 1;
        if target.broken {
            a.target_broken += 1;
        }
    }
    if baseline.actual_dest_arrival.is_some() && target.actual_dest_arrival.is_some() {
        /*if (target.actual_dest_arrival.unwrap() as f32-baseline.actual_dest_arrival.unwrap() as f32).abs() > 300.0 {
            println!("{:?} {:?} {:?} {:?} {:?}", meta.pair, meta.from_station_name, meta.to_station_name, target, baseline.actual_dest_arrival);
            println!("{:?}\n", baseline);
        }*/
        a.delta_baseline_target_predicted.push(target.original_dest_arrival_prediction-baseline.original_dest_arrival_prediction);
        a.delta_baseline_predicted_target_actual.push(target.actual_dest_arrival.unwrap() as f32-baseline.original_dest_arrival_prediction);
        a.delta_baseline_target_actual_arrival.push(target.actual_dest_arrival.unwrap() as f32-baseline.actual_dest_arrival.unwrap() as f32);
        a.delta_baseline_target_actual_travel_time.push(((target.actual_dest_arrival.unwrap()-target.departure)-(baseline.actual_dest_arrival.unwrap()-baseline.departure)) as f32);
    } else if baseline.actual_dest_arrival.is_none() && target.actual_dest_arrival.is_none() {
        a.baseline_and_target_infeasible += 1;
    }
}

fn summary(values: Vec<f32>, name: &str) {
    let mut arr = ndarray::Array::from_vec(values);
    let q5 = arr.quantile_axis_skipnan_mut(ndarray::Axis(0), n64(0.05), &Lower).unwrap();
    let q50 = arr.quantile_axis_skipnan_mut(ndarray::Axis(0), n64(0.5), &Nearest).unwrap();
    let q95 = arr.quantile_axis_skipnan_mut(ndarray::Axis(0), n64(0.95), &Higher).unwrap();
    println!("{}: mean {} stddev {} min {} 5% {} 50% {} 95% {} max {}", name, arr.mean().unwrap(), arr.std(1.0), arr.min().unwrap(), q5, q50, q95, arr.max().unwrap());
}

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 3 {
        println!("Usage: simulation (run|analyze) FILE");
        return;
    }
    match args[1].as_str() {
        "run" => {
            Simulation::new(&args[2]).run_simulation().unwrap();
        },
        "analyze" => {
            analyze_simulation(&args[2], if args.len() > 3 {Some(&args[3])} else {None});
        },
        _ => println!("Usage: simulation (run|analyze) FILE") 
    };
}