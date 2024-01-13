#[macro_use]
extern crate rmp_serde as rmps;

use std::collections::HashMap;
use std::time::Duration;
use std::time::SystemTime;

use stost::connection;
use stost::distribution_store;
use stost::gtfs;
use stost::query::topocsa;
use stost::wire::serde;

use rmps::Serializer;
use std::fs;

use glob::glob;

use criterion::{black_box, criterion_group, criterion_main, Criterion};

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
    let mut env = topocsa::new(
        &mut store,
        &mut tt.connections,
        &tt.stations,
        tt.cut,
        tt.labels,
        0,
        0.01,
        true,
    );

    let start_time = 8100;
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

                let connidx_dest0 = resolve_connection_idx(
                    &deterministic,
                    0,
                    deterministic.journeys[0].legs.len() - 2,
                    true,
                    &tt.transport_and_day_to_connection_id,
                    &env.order,
                );
                let connidx_dest1 = resolve_connection_idx(
                    &deterministic,
                    1,
                    deterministic.journeys[1].legs.len() - 2,
                    true,
                    &tt.transport_and_day_to_connection_id,
                    &env.order,
                );
                let connidx_dest2 = resolve_connection_idx(
                    &deterministic,
                    2,
                    deterministic.journeys[2].legs.len() - 2,
                    true,
                    &tt.transport_and_day_to_connection_id,
                    &env.order,
                );
                let connidx_dep0 = resolve_connection_idx(
                    &deterministic,
                    0,
                    0,
                    false,
                    &tt.transport_and_day_to_connection_id,
                    &env.order,
                );
                let connidx_dep1 = resolve_connection_idx(
                    &deterministic,
                    1,
                    0,
                    false,
                    &tt.transport_and_day_to_connection_id,
                    &env.order,
                );
                let connidx_dep2 = resolve_connection_idx(
                    &deterministic,
                    2,
                    0,
                    false,
                    &tt.transport_and_day_to_connection_id,
                    &env.order,
                );

                println!(
                    "debgu start:{} dd: {} {} dd: {} {} dd: {} {}",
                    deterministic.journeys[0].start_time,
                    tt.connections[connidx_dep0].departure.projected(),
                    tt.connections[connidx_dest0].arrival.projected(),
                    tt.connections[connidx_dep1].departure.projected(),
                    tt.connections[connidx_dest1].arrival.projected(),
                    tt.connections[connidx_dep2].departure.projected(),
                    tt.connections[connidx_dest2].arrival.projected(),
                );

                assert_eq!(
                    tt.connections[connidx_dest0].arrival.scheduled,
                    deterministic.journeys[0].dest_time
                );
                assert_eq!(tt.connections[connidx_dest0].to_idx, pair.1);

                let origin_deps = &station_labels[&pair.0];
                let mut i = 0;
                for dep in origin_deps.iter().rev() {
                    let c = &tt.connections[dep.connection_idx];
                    let cc = &tt.connections[connidx_dep1];
                    i += 1;
                    if c.departure.projected() >= minutes {
                        println!(
                            "dest prediction: raptor: {} {} topocsa: {} {} i: {}",
                            cc.departure.projected(),
                            deterministic.journeys[0].dest_time,
                            c.departure.projected(),
                            c.destination_arrival.borrow().as_ref().unwrap().mean,
                            i
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

fn dummy(i: i32) -> i32 {
    //println!("dummy");
    i * 2
}

pub fn simulation(c: &mut Criterion) {
    let r = setup().unwrap();
    /*let mut group = c.benchmark_group("once");
    group.sample_size(10); //measurement_time(Duration::from_secs(10))
    group.bench_function("basic", |b| b.iter(|| dummy(black_box(r))));
    group.finish();*/
}

criterion_group!(benches, simulation);
criterion_main!(benches);
