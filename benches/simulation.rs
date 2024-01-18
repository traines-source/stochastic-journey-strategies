use criterion::{black_box, criterion_group, Criterion};
use glob::glob;
use motis_nigiri::Timetable;
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

#[derive(Serialize, Deserialize, Debug)]
struct SimulationConfig {
    distributions_path: String,
    gtfs_path: String,
    gtfs_cache_path: String,
    gtfsrt_glob: String,
    det_simulation: String,
    stoch_simulation: String,
    transfer: String,
    samples: usize
}

fn load_config(path: &str) -> SimulationConfig {
    let buf = std::fs::read(path).unwrap();
    serde_json::from_slice(&buf).unwrap()
}

fn load_samples() -> Vec<OriginDestinationSample> {
    let buf = std::fs::read("./benches/samples/samples.json").unwrap();
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
    order: &HashMap<usize, topocsa::ConnectionOrder>,
) -> usize {
    let leg = &journey.legs[leg_idx];
    let connid = mapping[&(leg.transport_idx, leg.day_idx)]
        + if to {
            leg.to_stop_idx - 1
        } else {
            leg.from_stop_idx
        } as usize;
    order[&connid].order
}

fn update_footpaths(t: &Timetable, tt: &mut GtfsTimetable) {
    let mut i = 0;
    for s in &mut tt.stations {
        s.footpaths.clear();
        s.footpaths.append(&mut t.get_location(i).footpaths);
        i += 1;
    }
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
            0,
            0.01,
            true,
        );
        gtfs::load_realtime(
            &path,
            &t,
            &tt.transport_and_day_to_connection_id,
            |connection_id: usize, is_departure: bool, delay: i16, cancelled: bool| {
                env.update(connection_id, is_departure, delay, cancelled)
            },
        );
        let station_labels = env.query(&tt.stations[pair.0], &tt.stations[pair.1]);
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
                        let connidx_dep0 = tt.order[&connid].order;
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
        let origin_deps = &station_labels[&pair.0];
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
    proj_dest_arr: f32
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
    to_station: String,
    det: SimulationResult,
    stoch: SimulationResult
}

impl SimulationJourney {
    fn is_completed(&self) -> bool {
        self.det.is_completed() && self.stoch.is_completed()
    }
}

#[derive(Serialize, Deserialize, Debug)]
struct SimulationRun {
    simulation_run_at: u64,
    config: SimulationConfig,
    results: Vec<SimulationJourney>
}

fn run_simulation() -> Result<i32, Box<dyn std::error::Error>> {
    let conf = load_config("./benches/config/config.json");

    let mut store = distribution_store::Store::new();
    store.load_distributions(&conf.distributions_path);

    let mut tt = gtfs::load_gtfs_cache(&conf.gtfs_cache_path);
    let t = gtfs::load_timetable(&conf.gtfs_path, day(2023, 11, 1), day(2023, 11, 2));
    //update_footpaths(&t, &mut tt);
    let start_ts = t.get_start_day_ts() as u64;

    let start_time = 8100;
    let stop_pairs: Vec<(usize, usize, i32)> = load_samples().into_iter().take(conf.samples).map(|s| (s.from_idx, s.to_idx, start_time)).collect();
    let mut det_actions: HashMap<(usize, usize, i32), motis_nigiri::Journey> = HashMap::new();
    let mut stoch_actions: HashMap<(usize, usize, i32), HashMap<usize, Vec<topocsa::ConnectionLabel>>> = HashMap::new();
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
            0,
            0.01,
            true,
        );
        gtfs::load_realtime(
            &path,
            &t,
            &tt.transport_and_day_to_connection_id,
            |connection_id: usize, is_departure: bool, delay: i16, cancelled: bool| {
                env.update(connection_id, is_departure, delay, cancelled)
            },
        );
        let mut do_continue = false;
        for pair in &stop_pairs {
            if is_first {
                let mut env = topocsa::new(
                    &mut store,
                    &mut tt.connections,
                    &tt.stations,
                    tt.cut.clone(),
                    &mut tt.order,
                    0,
                    0.01,
                    true,
                );
                let start = Instant::now();
                let stoch = env.query(&tt.stations[pair.0], &tt.stations[pair.1]);
                let timing_stoch = start.elapsed().as_millis();
                let start = Instant::now();
                let min_journey = t.get_journeys(pair.0, pair.1, current_time, false).journeys.into_iter().reduce(|a, b| if a.dest_time < b.dest_time {a} else {b});
                let timing_det = start.elapsed().as_millis();
                if min_journey.is_none() && stoch[&pair.0].len() == 0 {
                    println!("Infeasible for both det and stoch, skipping. pair: {:?}", pair);
                } else if min_journey.is_none() || stoch[&pair.0].len() == 0 {
                    println!("Infeasible for either det or stoch, skipping. det: {:?} stoch: {:?}", min_journey, stoch[&pair.0].len());
                } else {
                    det_actions.insert(*pair, min_journey.unwrap());
                    stoch_actions.insert(*pair, stoch);
                }
                det_log.insert(*pair, vec![]);
                stoch_log.insert(*pair, vec![]);
                results.insert(*pair, SimulationJourney {
                    pair: *pair,
                    start_time: start_ts+pair.2 as u64*60,
                    from_station: tt.stations[pair.0].id.clone(),
                    to_station: tt.stations[pair.1].id.clone(),
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
            let current_stop_idx = get_current_stop_idx(current_time, *pair, stoch_log.get_mut(pair).unwrap(), &mut results.get_mut(pair).unwrap().stoch, &tt);
            if current_stop_idx.is_some() {
                let alternatives = get_stoch_alternatives(current_stop_idx.unwrap(), &tt, &stoch_actions[pair]);
                step(current_time, pair.2, current_stop_idx.unwrap(), &alternatives, stoch_log.get_mut(pair).unwrap(), &mut results.get_mut(pair).unwrap().stoch, &tt.connections, &tt.stations);
            }
            let current_stop_idx = get_current_stop_idx(current_time, *pair, det_log.get_mut(pair).unwrap(), &mut results.get_mut(pair).unwrap().det, &tt);
            if current_stop_idx.is_some() {
                let alternatives = get_det_alternatives(current_stop_idx.unwrap(), &tt, &det_actions[pair]);
                step(current_time, pair.2, current_stop_idx.unwrap(), &alternatives, det_log.get_mut(pair).unwrap(), &mut results.get_mut(pair).unwrap().det, &tt.connections, &tt.stations);
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
    let filename = format!("./benches/runs/{}.{}.{}.{}.json", simulation_run_at, conf.det_simulation, conf.stoch_simulation, conf.transfer);
    let run = SimulationRun {
        simulation_run_at: simulation_run_at,
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
        return None;
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
                    update_connections_taken_from_last_log(result, log, &tt.connections);
                    result.actual_dest_arrival = Some(last_c.arrival.projected());
                }
                return None;
            }
            i += 1;
        }
        return Some(current_stop_idx);
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
    let departure_idx = resolve_connection_idx(det_actions, next_leg, false, &tt.transport_and_day_to_connection_id, &tt.order);
    let arrival_idx = resolve_connection_idx(det_actions, next_leg, true, &tt.transport_and_day_to_connection_id, &tt.order);
    let alternatives = vec![Alternative{
        from_conn_idx: departure_idx,
        to_conn_idx: arrival_idx,
        proj_dest_arr: det_actions.dest_time as f32
    }];
    alternatives
}

fn get_stoch_alternatives(current_stop_idx: usize, tt: &GtfsTimetable, stoch_actions: &HashMap<usize, Vec<topocsa::ConnectionLabel>>) -> Vec<Alternative> {
    let mut alternatives: Vec<Alternative> = vec![];
    let mut i = 0;
    let footpaths = &tt.stations[current_stop_idx].footpaths;
    while i <= footpaths.len() {
        let stop_idx = if i == footpaths.len() { current_stop_idx } else { footpaths[i].target_location_idx };
        alternatives.extend(stoch_actions[&stop_idx].iter().filter_map(|l| {
            if l.destination_arrival.mean == 0.0 {
                panic!("weirdly 0");
            }
            if l.destination_arrival.feasible_probability < 0.5 {
                return None // TODO properly use transfer strategy?
            }
            Some(Alternative{
                from_conn_idx: l.connection_idx,
                to_conn_idx: l.connection_idx,
                proj_dest_arr: l.destination_arrival.mean
            })
        }));
            
        i += 1;
    }
    alternatives.sort_by(|a, b| a.proj_dest_arr.partial_cmp(&b.proj_dest_arr).unwrap());
    alternatives
}

fn step(current_time: i32, start_time: i32, current_stop_idx: usize, alternatives: &[Alternative], log: &mut Vec<LogEntry>, result: &mut SimulationResult, connections: &[connection::Connection], stations: &[connection::Station]) {
    let arrival_time = if log.len() == 0 {start_time} else {connections[log.last().unwrap().conn_idx].arrival.projected()};
    let mut alternatives_still_available = false;
    if current_time >= arrival_time {
        for alt in alternatives {
            let next_c = &connections[alt.from_conn_idx];
            let transfer_time = if log.len() == 0 {0} else {get_transfer_time(current_stop_idx, next_c.from_idx, stations)}; // TODO intiial footpaths? 
            if next_c.departure.projected() >= arrival_time+transfer_time || (log.len() > 0 && connections[log.last().unwrap().conn_idx].is_consecutive(next_c)) {    
                if current_time >= next_c.departure.projected() { // TODO require not too long ago?
                    if log.len() > 0 {
                        update_connections_taken_from_last_log(result, log, connections);
                    } else {
                        result.departure = next_c.departure.projected();
                        result.original_dest_arrival_prediction = alt.proj_dest_arr;
                    }
                    update_connections_taken(result, &next_c, alt.proj_dest_arr);
                    log.push(LogEntry{
                        conn_idx: alt.to_conn_idx,
                        proj_dest_arr: alt.proj_dest_arr
                    });
                }
                alternatives_still_available = true;
                break;
            }
        }
        if !alternatives_still_available {
            if log.len() > 0 {
                update_connections_taken_from_last_log(result, log, connections);
            }
            if alternatives.len() > 0 {
                result.connection_missed = Some(connections[alternatives.last().unwrap().from_conn_idx].clone())
            }
            result.broken = true;
        }
    }
}

fn update_connections_taken_from_last_log(result: &mut SimulationResult, log: &[LogEntry], connections: &[connection::Connection]) {
    update_connections_taken(result, &connections[log.last().unwrap().conn_idx], log.last().unwrap().proj_dest_arr);
}

fn update_connections_taken(result: &mut SimulationResult, connection: &connection::Connection, proj_dest_arr: f32) {
    let conn = connection.clone();
    conn.destination_arrival.replace(Some(distribution::Distribution {
        feasible_probability: 1.0,
        histogram: vec![],
        start: 0,
        mean: proj_dest_arr
    }));
    result.connections_taken.push(conn);
    result.broken = false;
}

fn get_transfer_time(from_stop_idx: usize, to_stop_idx: usize, stations: &[connection::Station]) -> i32 {
    if from_stop_idx == to_stop_idx {
        return 1;
    }
    for f in &stations[from_stop_idx].footpaths {
        if f.target_location_idx == to_stop_idx {
            return f.duration as i32;
        }
    }
    panic!("Tried walking where no walking connection exists from {} to {}", from_stop_idx, to_stop_idx);
}

#[ignore]
pub fn simulation(c: &mut Criterion) {
    run_simulation().unwrap();
    //manual_test().unwrap();
    /*let mut group = c.benchmark_group("once");
    group.sample_size(10); //measurement_time(Duration::from_secs(10))
    group.bench_function("basic", |b| b.iter(|| dummy(black_box(r))));
    group.finish();*/
}

criterion_group!(benches, simulation);
