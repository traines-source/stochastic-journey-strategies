use criterion::{black_box, criterion_group, Criterion};
use glob::glob;
use serde::Deserialize;
use serde::Serialize;
use std::collections::HashMap;
use std::fs;
use std::time::SystemTime;
use stost::distribution_store;
use stost::gtfs;
use stost::query::topocsa;

#[derive(Serialize, Deserialize, Debug)]
struct SimulationConfig {
    distributions_path: String,
    gtfs_path: String,
    gtfs_cache_path: String,
    gtfsrt_glob: String,
}

struct SimResult {
    original_dest_arrival_prediction: i32,
    actual_dest_arrival: i32,
    broken: bool,
}

fn load_config(path: &str) -> SimulationConfig {
    let buf = std::fs::read(path).unwrap();
    serde_json::from_slice(&buf).unwrap()
}

fn day(year: i32, month: u32, day: u32) -> chrono::NaiveDate {
    chrono::NaiveDate::from_ymd_opt(year, month, day).unwrap()
}

fn resolve_connection_idx(
    pareto_set: &motis_nigiri::ParetoSet,
    journex_idx: usize,
    leg_idx: usize,
    to: bool,
    mapping: &HashMap<(usize, u16), usize>,
    order: &HashMap<usize, topocsa::ConnectionOrder>,
) -> usize {
    let leg = &pareto_set.journeys[journex_idx].legs[leg_idx];
    let connid = mapping[&(leg.transport_idx, leg.day_idx)]
        + if to {
            leg.to_stop_idx - 1
        } else {
            leg.from_stop_idx
        } as usize;
    order[&connid].order
}

fn setup() -> Result<(i32), Box<dyn std::error::Error>> {
    let conf = load_config("./benches/config/config.json");

    let mut store = distribution_store::Store::new();
    store.load_distributions(&conf.distributions_path);

    let mut tt = gtfs::load_gtfs_cache(&conf.gtfs_cache_path);
    let t = gtfs::load_timetable(&conf.gtfs_path, day(2023, 11, 1), day(2023, 11, 2));
    let start_ts = t.get_start_day_ts() as u64;
    

    let start_time = 8140;
    let stop_pairs: Vec<(usize, usize)> = vec![(10000, 20000)];
    //let mut det_results = vec![];
    //let mut stoch_results = vec![];
    let mut is_first = true;
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
            &mut tt.labels,
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
        for pair in &stop_pairs {
            if is_first {
                let station_labels = env.query(&tt.stations[pair.0], &tt.stations[pair.1]);
                let deterministic = t.get_journeys(pair.0, pair.1, minutes, false);

                let mut i = 0;
                for j in &deterministic.journeys {
                    let connidx_dep0 = resolve_connection_idx(
                        &deterministic,
                        i,
                        0,
                        false,
                        &tt.transport_and_day_to_connection_id,
                        &tt.labels,
                    );
                    let connidx_dest0 = resolve_connection_idx(
                        &deterministic,
                        i,
                        deterministic.journeys[0].legs.len() - 2,
                        true,
                        &tt.transport_and_day_to_connection_id,
                        &tt.labels,
                    );
                    let mut last_connid = 0;
                    for k in 0..j.legs.len() {
                        let leg = &j.legs[k];
                        if !leg.is_footpath {
                            println!("leg {} {} {}", leg.from_location_idx, leg.to_location_idx, leg.transport_idx);
                            println!("route {:?}", t.get_route(t.get_transport(leg.transport_idx).route_idx));
                            for l in leg.from_stop_idx..(leg.to_stop_idx+1) {
                                let connid = tt.transport_and_day_to_connection_id[&(leg.transport_idx, leg.day_idx)]+l as usize-1;
                                let connidx_dep0 = tt.labels[&connid].order;
                                println!(
                                    "debgu start:{} {} {} frm dd: frmcon {} {} {} tocon {} {} {} trip: {} {} {} mean {} cut: {} fp: {}",
                                    deterministic.journeys[0].start_time, i, connid,
                                    tt.connections[connidx_dep0].from_idx,
                                    tt.stations[tt.connections[connidx_dep0].from_idx].name,
                                    tt.connections[connidx_dep0].departure.projected(),
                                    tt.connections[connidx_dep0].to_idx,
                                    tt.stations[tt.connections[connidx_dep0].to_idx].name,
                                    tt.connections[connidx_dep0].arrival.projected(),
                                    tt.connections[connidx_dep0].route_idx,
                                    tt.connections[connidx_dep0].trip_id,
                                    connidx_dep0, tt.connections[connidx_dep0].destination_arrival.borrow().as_ref().unwrap().mean,
                                    tt.cut.contains(&(last_connid, connid)),
                                    tt.stations[tt.connections[connidx_dep0].from_idx].footpaths.len()
                                );
                                last_connid = connid;
                            }
                        } else {
                            println!("footpath");
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
            }
            break;
        }
        is_first = false;
        break;
    }
    Ok(5)
}

#[ignore]
pub fn simulation(c: &mut Criterion) {
    let r = setup().unwrap();
    /*let mut group = c.benchmark_group("once");
    group.sample_size(10); //measurement_time(Duration::from_secs(10))
    group.bench_function("basic", |b| b.iter(|| dummy(black_box(r))));
    group.finish();*/
}

criterion_group!(benches, simulation);
