use criterion::{black_box, criterion_group, Criterion};
use glob::glob;
use motis_nigiri::Timetable;
use ndarray_stats::Quantile1dExt;
use ndarray_stats::QuantileExt;
use ndarray_stats::interpolate::Linear;
use serde::Deserialize;
use serde::Serialize;
use stost::connection;
use stost::gtfs::GtfsTimetable;
use stost::gtfs::OriginDestinationSample;
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

#[derive(Serialize, Deserialize, Debug)]
struct SimulationConfig {
    distributions_path: String,
    gtfs_path: String,
    gtfs_cache_path: String,
    gtfsrt_glob: String,
    samples_config_path: String,
    det_simulation: String,
    stoch_simulation: String,
    transfer: String,
    samples: usize
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

fn day(year: i32, month: u32, day: u32) -> chrono::NaiveDate {
    chrono::NaiveDate::from_ymd_opt(year, month, day).unwrap()
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

fn manual_test() -> Result<i32, Box<dyn std::error::Error>> {
    let conf = load_config("./benches/config/config.json");

    let mut store = distribution_store::Store::new();
    store.load_distributions(&conf.distributions_path);

    let mut tt = gtfs::load_gtfs_cache(&conf.gtfs_cache_path);
    let t = gtfs::load_timetable(&conf.gtfs_path, day(2023, 11, 1), day(2023, 11, 2));
    let start_ts = t.get_start_day_ts() as u64;

    let start_time = 8100;
    let pair = (10000, 20000);
    for f in glob(&conf.gtfsrt_glob).expect("Failed to read glob pattern") {
        let path = f.as_ref().unwrap().to_str().unwrap().to_owned();
        let minutes = ((fs::metadata(f?)?
            .modified()?
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs()
            - start_ts)
            / 60) as i32;
        if minutes < start_time {
            continue;
        }
        let mut env = topocsa::new(
            &mut store,
            &mut tt.connections,
            &tt.stations,
            tt.cut.clone(),
            &mut tt.order,
            start_time,
            0.01,
            true,
        );
        gtfs::load_realtime(
            &path,
            &t,
            &tt.transport_and_day_to_connection_id,
            |connection_id: usize, is_departure: bool, location_idx: Option<usize>, in_out_allowed: Option<bool>, delay: Option<i16>| {
                env.update(connection_id, is_departure, location_idx, in_out_allowed, delay)
            },
        );
        let station_labels = env.query(pair.0, pair.1, start_time, start_time+1440);
        let deterministic = t.get_journeys(pair.0, pair.1, minutes, false);

        let mut i = 0;
        for j in &deterministic.journeys {
            let mut last_connid = 0;
            for k in 0..j.legs.len() {
                let leg = &j.legs[k];
                if !leg.is_footpath {
                    println!("leg {} {} {}", leg.from_location_idx, leg.to_location_idx, leg.transport_idx);
                    println!("route {:?}", t.get_route(t.get_transport(leg.transport_idx).route_idx));
                    for l in leg.from_stop_idx..(leg.to_stop_idx+1) {
                        let connid = tt.transport_and_day_to_connection_id[&(leg.transport_idx, leg.day_idx)]+l as usize-1;
                        let connidx_dep0 = tt.order[connid];
                        println!(
                            "debgu start:{} {} {} frm dd: frmcon {} {} {} {:?} tocon {} {} {} {:?} trip: {} {} {} mean {} cut: {} fp: {}",
                            deterministic.journeys[0].start_time, i, connid,
                            tt.connections[connidx_dep0].from_idx,
                            tt.stations[tt.connections[connidx_dep0].from_idx].name,
                            tt.connections[connidx_dep0].departure.projected(),
                            tt.connections[connidx_dep0].departure.delay,
                            tt.connections[connidx_dep0].to_idx,
                            tt.stations[tt.connections[connidx_dep0].to_idx].name,
                            tt.connections[connidx_dep0].arrival.projected(),
                            tt.connections[connidx_dep0].arrival.delay,
                            tt.connections[connidx_dep0].route_idx,
                            tt.connections[connidx_dep0].trip_id,
                            connidx_dep0, tt.connections[connidx_dep0].destination_arrival.borrow().as_ref().unwrap().mean,
                            tt.cut.contains(&(last_connid, connid)),
                            tt.stations[tt.connections[connidx_dep0].from_idx].footpaths.len()
                        );
                        last_connid = connid;
                    }
                } else {
                    println!("footpath, dur: {}", leg.duration);
                }
            }
            i += 1;
        }
        let origin_deps = &station_labels[pair.0];
        let mut i = 0;
        for dep in origin_deps.iter().rev() {
            let c = &tt.connections[dep.connection_idx];
            i += 1;
            if c.departure.projected() >= minutes {
                println!(
                    "dest prediction:topocsa: {} {} {} {} i: {} {}",
                    c.departure.projected(),
                    start_ts+c.departure.projected() as u64*60,
                    c.destination_arrival.borrow().as_ref().unwrap().mean,
                    start_ts+c.destination_arrival.borrow().as_ref().unwrap().mean as u64*60,
                    i, dep.connection_idx
                );
            }
        }
        break;
    }
    Ok(0)
}

struct LogEntry {
    conn_idx: usize,
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
    connection_pairs: HashMap<usize, usize>,
    connection_pairs_reverse: HashMap<usize, usize>
}

fn run_simulation() -> Result<i32, Box<dyn std::error::Error>> {
    let conf = load_config("./benches/config/config.json");

    let mut store = distribution_store::Store::new();
    store.load_distributions(&conf.distributions_path);

    let mut tt = gtfs::load_gtfs_cache(&conf.gtfs_cache_path);
    let t = gtfs::load_timetable(&conf.gtfs_path, day(2023, 11, 2), day(2023, 11, 3));
    if conf.transfer == "short" {
        gtfs::shorten_footpaths(&mut tt.stations);
    }
    let start_ts = t.get_start_day_ts() as u64;

    let start_time = 8050;
    let stop_pairs: Vec<(usize, usize, i32)> = load_samples(&conf.samples_config_path).into_iter().take(conf.samples).map(|s| (s.from_idx, s.to_idx, start_time)).collect();
    //let stop_pairs = vec![(19337, 37262, 8050), (1423, 21135, 8050)];
    let mut det_actions: HashMap<(usize, usize, i32), motis_nigiri::Journey> = HashMap::new();
    let mut stoch_actions: HashMap<(usize, usize, i32), StochActions> = HashMap::new();
    let mut det_log: HashMap<(usize, usize, i32), Vec<LogEntry>> = HashMap::new();
    let mut stoch_log: HashMap<(usize, usize, i32), Vec<LogEntry>> = HashMap::new();
    let mut results: HashMap<(usize, usize, i32), SimulationJourney> = HashMap::new();
    let mut is_first = true;
    let simulation_run_at = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
    for f in glob(&conf.gtfsrt_glob).expect("Failed to read glob pattern") {
        let path = f.as_ref().unwrap().to_str().unwrap().to_owned();
        let current_time = ((fs::metadata(f?)?
            .modified()?
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs()
            - start_ts)
            / 60) as i32;
        if current_time < start_time {
            continue;
        }
        let mut env = topocsa::new(
            &mut store,
            &mut tt.connections,
            &tt.stations,
            tt.cut.clone(),
            &mut tt.order,
            start_time,
            0.01,
            true,
        );
        println!("Loading {}", path);
        gtfs::load_realtime(
            &path,
            &t,
            &tt.transport_and_day_to_connection_id,
            |connection_id: usize, is_departure: bool, location_idx: Option<usize>, in_out_allowed: Option<bool>, delay: Option<i16>| {
                env.update(connection_id, is_departure, location_idx, in_out_allowed, delay)
            },
        );
        let mut do_continue = false;
        for pair in &stop_pairs {
            println!("Pair: {:?}", pair);
            if is_first {
                let mut env = topocsa::new(
                    &mut store,
                    &mut tt.connections,
                    &tt.stations,
                    tt.cut.clone(),
                    &mut tt.order,
                    start_time,
                    0.01,
                    true,
                );
                let start = Instant::now();
                let stoch = env.query(pair.0, pair.1, start_time, start_time+1440);
                let timing_stoch = start.elapsed().as_millis();
                let start = Instant::now();
                let min_journey = t.get_journeys(pair.0, pair.1, current_time, false).journeys.into_iter().reduce(|a, b| if a.dest_time < b.dest_time {a} else {b});
                let timing_det = start.elapsed().as_millis();
                if min_journey.is_none() || !stoch.get(pair.0).is_some_and(|s| s.len() > 0) {
                    println!("Infeasible for either det or stoch, skipping. det: {:?} stoch: {:?}", min_journey, stoch.get(pair.0).is_some_and(|s| s.len() > 0));
                } else {
                    det_actions.insert(*pair, min_journey.unwrap());
                    let mut relevant_stations = env.relevant_stations(pair.2, pair.0, pair.1, &stoch);
                    println!("Enriching relevant stations...");
                    for l in &det_actions[pair].legs {
                        println!("{} {} {:?}", l.from_location_idx, tt.stations[l.from_location_idx].name, relevant_stations.insert(l.from_location_idx, 1000.0));
                        println!("{} {} {:?}", l.to_location_idx, tt.stations[l.to_location_idx].name, relevant_stations.insert(l.to_location_idx, 1000.0));
                    }
                    let relevant_pairs = env.relevant_connection_pairs(relevant_stations);
                    stoch_actions.insert(*pair, StochActions{
                        station_labels: stoch,
                        connection_pairs_reverse: relevant_pairs.iter().map(|(k,v)| (*v,*k)).collect(),
                        connection_pairs: relevant_pairs
                    });
                }
                det_log.insert(*pair, vec![]);
                stoch_log.insert(*pair, vec![]);
                results.insert(*pair, SimulationJourney {
                    pair: *pair,
                    start_time: start_ts+pair.2 as u64*60,
                    from_station: tt.stations[pair.0].id.clone(),
                    from_station_name: tt.stations[pair.0].name.clone(),
                    to_station: tt.stations[pair.1].id.clone(),
                    to_station_name: tt.stations[pair.1].name.clone(),
                    det: SimulationResult {
                        departure: 0,
                        original_dest_arrival_prediction: 0.0,
                        actual_dest_arrival: None,
                        algo_elapsed_ms: vec![timing_det],
                        broken: false,
                        connections_taken: vec![],
                        connection_missed: None
                    },
                    stoch: SimulationResult {
                        departure: 0,
                        original_dest_arrival_prediction: 0.0,
                        actual_dest_arrival: None,
                        algo_elapsed_ms: vec![timing_stoch],
                        broken: false,
                        connections_taken: vec![],
                        connection_missed: None                        
                    }
                });
            }
            if det_actions.get(pair).is_none() || stoch_actions.get(pair).is_none() {
                continue;
            }
            if conf.det_simulation == "priori_online_broken" {
                if results[pair].det.broken {
                    let stuck_at = det_log[pair].last().map(|l| tt.connections[l.conn_idx].to_idx).unwrap_or(pair.0);
                    let min_journey = t.get_journeys(stuck_at, pair.1, current_time, false).journeys.into_iter().reduce(|a, b| if a.dest_time < b.dest_time {a} else {b});
                    if min_journey.is_some() {
                        println!("Replacing broken det itinerary.");
                        det_actions.insert(*pair, min_journey.unwrap());
                        results.get_mut(pair).unwrap().det.broken = false;
                    }
                }
            }
            if conf.stoch_simulation == "adaptive_online_relevant" {
                println!("relevant topocsa...");
                let mut env = topocsa::new(
                    &mut store,
                    &mut tt.connections,
                    &tt.stations,
                    tt.cut.clone(),
                    &mut tt.order,
                    current_time,
                    0.01,
                    true,
                );
                let stoch = env.pair_query(pair.0, pair.1, start_time, start_time+1440, &stoch_actions[pair].connection_pairs);
                stoch_actions.get_mut(pair).unwrap().station_labels = stoch;
                println!("relevant topocsa done.");
            }
            let mut repeat = true;
            while repeat {
                repeat = false;
                let current_stop_idx = get_current_stop_idx(current_time, *pair, stoch_log.get_mut(pair).unwrap(), &mut results.get_mut(pair).unwrap().stoch, &tt);
                if current_stop_idx.is_some() {
                    let alternatives = get_stoch_alternatives(current_stop_idx.unwrap(), &tt, &stoch_actions[pair]);
                    repeat = step(current_time, pair.2, current_stop_idx.unwrap(), &alternatives, stoch_log.get_mut(pair).unwrap(), &mut results.get_mut(pair).unwrap().stoch, &tt.connections, &tt.stations);
                }
            }
            repeat = true;
            while repeat {
                repeat = false;
                let current_stop_idx = get_current_stop_idx(current_time, *pair, det_log.get_mut(pair).unwrap(), &mut results.get_mut(pair).unwrap().det, &tt);
                if current_stop_idx.is_some() {
                    let alternatives = get_det_alternatives(current_stop_idx.unwrap(), &tt, &det_actions[pair]);
                    repeat = step(current_time, pair.2, current_stop_idx.unwrap(), &alternatives, det_log.get_mut(pair).unwrap(), &mut results.get_mut(pair).unwrap().det, &tt.connections, &tt.stations);
                    let cidx = det_log[pair].last();
                    if cidx.is_some() && !stoch_actions[pair].connection_pairs.contains_key(&cidx.unwrap().conn_idx) && !stoch_actions[pair].connection_pairs_reverse.contains_key(&cidx.unwrap().conn_idx) {
                        panic!("WARN: connection from det not contained in connection pairs {} {} {}", cidx.unwrap().conn_idx, tt.connections[cidx.unwrap().conn_idx].from_idx, tt.connections[cidx.unwrap().conn_idx].to_idx);
                    }
                }
            }
            if !results[pair].is_completed() {
                do_continue = true;
            }
        }
        is_first = false;
        if !do_continue {
            println!("All simulations completed. Stopping for start_time {} at current_time {}.", start_time, current_time);
            break;
        }
    }
    let filename = format!("./benches/runs/{}.{}.{}.{}.ign.json", simulation_run_at, conf.det_simulation, conf.stoch_simulation, conf.transfer);
    let run = SimulationRun {
        simulation_run_at: simulation_run_at,
        comment: "".to_string(),
        config: conf,
        results: results.into_values().collect(),
    };
    let buf = serde_json::to_vec(&run).unwrap();
    let mut file = fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(filename).expect("file not openable");
    file.write_all(&buf).expect("error writing file");
    Ok(0)
}

fn get_current_stop_idx(current_time: i32, pair: (usize, usize, i32), log: &mut Vec<LogEntry>, result: &mut SimulationResult, tt: &GtfsTimetable) -> Option<usize> {
    if result.actual_dest_arrival.is_some() {
        None
    } else if log.len() == 0 {
        Some(pair.0)
    } else {
        let last_c = &tt.connections[log.last().unwrap().conn_idx];
        let current_stop_idx = last_c.to_idx;
        let footpaths = &tt.stations[current_stop_idx].footpaths;
        let mut i = 0;
        while i <= footpaths.len() {
            let stop_idx = if i == footpaths.len() { current_stop_idx } else { footpaths[i].target_location_idx };
            if stop_idx == pair.1 {
                if current_time >= last_c.arrival.projected() {
                    update_connections_taken_from_last_log(result, log, &tt.connections, &tt.stations);
                    result.actual_dest_arrival = Some(last_c.arrival.projected());
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
    if next_leg >= det_actions.legs.len() { // TODO platform change
        println!("failed to find journey continuation (platform change?): {:?} current_stop: {:?}", det_actions, tt.stations[current_stop_idx]);
        return vec![];
    }
    let departure_idx = resolve_connection_idx(det_actions, next_leg, false, &tt.transport_and_day_to_connection_id, &tt.order);
    let arrival_idx = resolve_connection_idx(det_actions, next_leg, true, &tt.transport_and_day_to_connection_id, &tt.order);
    let alternatives = vec![Alternative{
        from_conn_idx: departure_idx,
        to_conn_idx: arrival_idx,
        proj_dest_arr: det_actions.dest_time as f32
    }];
    alternatives
}

fn get_stoch_alternatives(current_stop_idx: usize, tt: &GtfsTimetable, stoch_actions: &StochActions) -> Vec<Alternative> {
    let mut alternatives: Vec<Alternative> = vec![];
    let footpaths = &tt.stations[current_stop_idx].footpaths;
    for i in 0..footpaths.len()+1 {
        let stop_idx = if i == footpaths.len() { current_stop_idx } else { footpaths[i].target_location_idx };
        let station_labels = stoch_actions.station_labels.get(stop_idx);
        if station_labels.is_none() {
            continue;
        }
        println!("label len: {} {} {} {}", current_stop_idx, stoch_actions.station_labels.iter().filter(|l| l.len() > 0).count(), station_labels.unwrap().len(), stoch_actions.connection_pairs.len());
        alternatives.extend(station_labels.unwrap().iter().filter_map(|l| {
            if l.destination_arrival.mean == 0.0 {
                panic!("weirdly 0");
            }
            if l.destination_arrival.feasible_probability < 0.5 || !tt.connections[l.connection_idx].departure.in_out_allowed {
                return None // TODO properly use transfer strategy?
            }
            Some(Alternative{
                from_conn_idx: l.connection_idx,
                to_conn_idx: stoch_actions.connection_pairs_reverse[&l.connection_idx],
                proj_dest_arr: l.destination_arrival.mean
            })
        }));
    }

    alternatives.sort_unstable_by(|a, b| a.proj_dest_arr.partial_cmp(&b.proj_dest_arr).unwrap());
    alternatives
}

fn step(current_time: i32, start_time: i32, current_stop_idx: usize, alternatives: &[Alternative], log: &mut Vec<LogEntry>, result: &mut SimulationResult, connections: &[connection::Connection], stations: &[connection::Station]) -> bool {
    let arrival_time = if log.len() == 0 {
        start_time
    } else {
        let c = &connections[log.last().unwrap().conn_idx];
        std::cmp::max(std::cmp::max(c.arrival.projected(), c.departure.projected()), log.last().unwrap().arrival_time_lower_bound) // TODO enforce while gtfsrt updating?
    };
    let mut alternatives_still_available = false;
    if current_time >= arrival_time {
        for alt in alternatives {
            let next_c = &connections[alt.from_conn_idx];
            let transfer_time = get_transfer_time(current_stop_idx, next_c.from_idx, log.is_empty(), stations); 
            if next_c.departure.projected() >= arrival_time+transfer_time || (log.len() > 0 && connections[log.last().unwrap().conn_idx].is_consecutive(next_c)) {    
                if current_time >= next_c.departure.projected() { // TODO require not too long ago?
                    if log.len() > 0 {
                        update_connections_taken_from_last_log(result, log, connections, stations);
                    } else {
                        result.departure = next_c.departure.projected();
                        result.original_dest_arrival_prediction = alt.proj_dest_arr;
                    }
                    update_connections_taken(result, &next_c, stations, alt.proj_dest_arr);
                    println!("step {} {} {} from/to: {} {} trip: {} arr: {} {} dep: {} {} to_conn: dp: {} {} arr: {} {} from/to: {} {} trip: {}", arrival_time, alt.from_conn_idx, alt.to_conn_idx, next_c.from_idx, next_c.to_idx, next_c.trip_id, next_c.arrival.scheduled, next_c.arrival.projected(), next_c.departure.scheduled, next_c.departure.projected(), connections[alt.to_conn_idx].departure.scheduled, connections[alt.to_conn_idx].departure.projected(), connections[alt.to_conn_idx].arrival.scheduled, connections[alt.to_conn_idx].arrival.projected(), connections[alt.to_conn_idx].from_idx, connections[alt.to_conn_idx].to_idx, connections[alt.to_conn_idx].trip_id);
                    log.push(LogEntry{
                        conn_idx: alt.to_conn_idx,
                        proj_dest_arr: alt.proj_dest_arr,
                        arrival_time_lower_bound: arrival_time
                    });
                    return true;
                }
                alternatives_still_available = true;
                break;
            }
        }
        if !alternatives_still_available && !result.broken {
            if log.len() > 0 {
                update_connections_taken_from_last_log(result, log, connections, stations);
            }
            if alternatives.len() > 0 {
                result.connection_missed = Some(connections[alternatives.last().unwrap().from_conn_idx].clone());
            }
            result.broken = true;
        }
    }
    return false;
}

fn update_connections_taken_from_last_log(result: &mut SimulationResult, log: &[LogEntry], connections: &[connection::Connection], stations: &[connection::Station]) {
    update_connections_taken(result, &connections[log.last().unwrap().conn_idx], stations, log.last().unwrap().proj_dest_arr);
}

fn update_connections_taken(result: &mut SimulationResult, connection: &connection::Connection, stations: &[connection::Station], proj_dest_arr: f32) {
    let mut conn = connection.clone();
    /*conn.destination_arrival.replace(Some(distribution::Distribution {
        feasible_probability: 1.0,
        histogram: vec![],
        start: 0,
        mean: proj_dest_arr
    }));*/
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

struct SimulationAnalysis {
    det_infeasible: i32,
    det_broken: i32,
    stoch_infeasible: i32,
    stoch_broken: i32,
    det_stoch_infeasible: i32,
    delta_det_predicted_actual: Vec<f32>,
    delta_stoch_predicted_actual: Vec<f32>,
    delta_det_stoch_predicted: Vec<f32>,
    delta_det_predicted_stoch_actual: Vec<f32>,
    delta_det_stoch_actual_arrival: Vec<f32>,
    delta_det_stoch_actual_travel_time: Vec<f32>,
    stoch_actual_travel_time: Vec<f32>
}

fn summary(values: Vec<f32>, name: &str) {
    let mut arr = ndarray::Array::from_vec(values);
    let q5 = arr.quantile_axis_skipnan_mut(ndarray::Axis(0), n64(0.05), &Linear).unwrap();
    let q95 = arr.quantile_axis_skipnan_mut(ndarray::Axis(0), n64(0.95), &Linear).unwrap();
    println!("{}: mean {} stddev {} min {} 5% {} max {} 95% {}", name, arr.mean().unwrap(), arr.std(1.0), arr.min().unwrap(), q5, arr.max().unwrap(), q95);
}

pub fn analyze_simulation() {
    let run = load_simulation_run("./benches/runs/1706891316.priori_online_broken.adaptive_online_relevant.short.ign.json");
    let mut a = SimulationAnalysis {
        det_infeasible: 0,
        det_broken: 0,
        stoch_infeasible: 0,
        stoch_broken: 0,
        det_stoch_infeasible: 0,
        delta_det_predicted_actual: vec![],
        delta_stoch_predicted_actual: vec![],
        delta_det_stoch_predicted: vec![],
        delta_det_predicted_stoch_actual: vec![],
        delta_det_stoch_actual_arrival: vec![],
        delta_det_stoch_actual_travel_time: vec![],
        stoch_actual_travel_time: vec![]
    };
    for result in &run.results {
        if result.det.actual_dest_arrival.is_some() {
            a.delta_det_predicted_actual.push(result.det.actual_dest_arrival.unwrap() as f32-result.det.original_dest_arrival_prediction);
        } else {
            a.det_infeasible += 1;
            if result.det.broken {
                a.det_broken += 1;
            }
        }
        if result.stoch.actual_dest_arrival.is_some() {
            a.delta_stoch_predicted_actual.push(result.stoch.actual_dest_arrival.unwrap() as f32-result.stoch.original_dest_arrival_prediction);
            a.stoch_actual_travel_time.push((result.stoch.actual_dest_arrival.unwrap()-result.stoch.departure) as f32);
        } else {
            a.stoch_infeasible += 1;
            if result.stoch.broken {
                a.stoch_broken += 1;
            }
        }
        if result.det.actual_dest_arrival.is_some() && result.stoch.actual_dest_arrival.is_some() {
            a.delta_det_stoch_predicted.push(result.stoch.original_dest_arrival_prediction-result.det.original_dest_arrival_prediction);
            a.delta_det_predicted_stoch_actual.push(result.stoch.actual_dest_arrival.unwrap() as f32-result.det.original_dest_arrival_prediction);
            a.delta_det_stoch_actual_arrival.push(result.stoch.actual_dest_arrival.unwrap() as f32-result.det.actual_dest_arrival.unwrap() as f32);
            a.delta_det_stoch_actual_travel_time.push(((result.stoch.actual_dest_arrival.unwrap()-result.stoch.departure)-(result.det.actual_dest_arrival.unwrap()-result.det.departure)) as f32);
        } else if result.det.actual_dest_arrival.is_none() && result.stoch.actual_dest_arrival.is_none() {
            a.det_stoch_infeasible += 1;
        }
    }
    println!("infeasible: both: {} det: {} stoch: {} broken: det: {} stoch: {} feasible: both: {}", a.det_stoch_infeasible, a.det_infeasible, a.stoch_infeasible, a.det_broken, a.stoch_broken, a.delta_det_stoch_actual_travel_time.len());
    summary(a.delta_det_predicted_actual, "delta_det_predicted_actual");
    summary(a.delta_stoch_predicted_actual, "delta_stoch_predicted_actual");
    summary(a.delta_det_stoch_predicted, "delta_det_stoch_predicted");
    summary(a.delta_det_predicted_stoch_actual, "delta_det_predicted_stoch_actual");
    summary(a.delta_det_stoch_actual_arrival, "delta_det_stoch_actual_arrival");
    summary(a.delta_det_stoch_actual_travel_time, "delta_det_stoch_actual_travel_time");
    summary(a.stoch_actual_travel_time, "stoch_actual_travel_time");
}

#[ignore]
pub fn simulation(c: &mut Criterion) {
    //run_simulation().unwrap();
    analyze_simulation();
    //manual_test().unwrap();
    /*let mut group = c.benchmark_group("once");
    group.sample_size(10); //measurement_time(Duration::from_secs(10))
    group.bench_function("basic", |b| b.iter(|| dummy(black_box(r))));
    group.finish();*/
}

criterion_group!(benches, simulation);
