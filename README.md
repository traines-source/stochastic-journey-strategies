# Stochastic Strategies for Public Transport Journeys Based on Realtime Delay Predictions (StoSt)

This project aims to provide an implementation of an algorithm for stochastic journey planning for public transport. It relies on the delay distributions obtained from [public-transport-statistics](https://github.com/traines-source/public-transport-statistics) and is a dependency of [time-space-train-planner](https://github.com/traines-source/time-space-train-planner), which provides the UI and middleware. It uses [nigiri](https://github.com/motis-project/nigiri) for loading GTFS(-RT) data.

Using the delay distributions and cancellation probabilities and a scheduled timetable (GTFS) with realtime delay predictions (GTFS-RT), the algorithm calculates entire destination arrival distributions for a user query. For in intuitive understanding, best see the prototype in conjunction with [time-space-train-planner](https://github.com/traines-source/time-space-train-planner) surfacing the resulting histograms at https://tespace.traines.eu.

The main algorithm is located in [src/query/topocsa.rs](https://github.com/traines-source/stochastic-journey-strategies/blob/master/src/query/topocsa.rs). Many important utilities, like the handling of the distributions themselves, of GTFS, and of walking connections, are located in the top [src/](https://github.com/traines-source/stochastic-journey-strategies/blob/master/src/) directory. For more insight on other integral parts of the code base, refer to the "Using StoSt" section below.

## Building StoSt
1. Clone this repository.
2. Inside the created directory, clone https://github.com/traines-source/motis-nigiri-rust, so that there is a folder `motis-nigiri-rust` inside this repository's root. Follow the instructions over at [motis-nigiri-rust](https://github.com/traines-source/motis-nigiri-rust) to initially build the C++ dependencies.
3. If you have glibc 2.34 on your system, you should be able to just run `cargo build` to build StoSt. If you don't, you can also use Docker (recommended). In that case:
    a. Run `build-docker.sh` in the root of this repository (or run the contained commands manually) 
    b. For subsequent runs of `cargo`, you can use `./run-docker.sh cargo`. You may want to adapt the contained `docker run` command to mount the directory containing your GTFS(-RT) files.

## Using StoSt
In many cases, you will want to obtain GTFS(-RT) data to load into StoSt. A number of feeds are archived at https://mirror.traines.eu. Then there are three main ways the core algorithm can be leveraged:

### API Usage
StoSt exposes a [Protobuf](https://protobuf.dev/)-API, which is used by [time-space-train-planner](https://github.com/traines-source/time-space-train-planner). The schema definition can be found in [src/wire/wire.proto](https://github.com/traines-source/stochastic-journey-strategies/blob/master/src/wire/wire.proto). It can be used to create API consumers for the language of your choice. In order to serve the api, the `api` binary must be run. It must be given a path to a configuration JSON, like that:

```
./target/release/api ./deployments/config.json
```

This is also the default behaviour of the Docker image. In this configuration file, an arbitrary amount of systems (i.e. regions, countries...) can be specified that should be provided by the API. Two major modes exist, which are governed by the `provide_timetable` flag: Either the relevant timetable is provided by the caller (`false`), which must necessarily be a very limited timetable based on the "relevant stops approach", or StoSt itself loads the timetable from GTFS(-RT) feeds and just receives the query via the API. In particular the latter mode is very prototypical at the moment. For instance, it does not yet automatically refresh the GTFS-RT feed. For more details, see the [config.json](https://github.com/traines-source/stochastic-journey-strategies/blob/master/deployments/config.json), the glue code in [src/bin/api.rs](https://github.com/traines-source/stochastic-journey-strategies/blob/master/src/bin/api.rs) and also [stost.go in time-space-train-planner](https://github.com/traines-source/time-space-train-planner/blob/master/src/internal/stost.go), which uses both modes depending on the system.

### Manual Usage/Usage from Code
For experimentation with single queries, the manual integration tests in [tests/gtfs.rs](https://github.com/traines-source/stochastic-journey-strategies/blob/master/tests/gtfs.rs) are helpful. They contain many examples on how to load GTFS and corresponding GTFS-RT files and running queries on them.
One can run single bootstrap tests e.g. like that:

```
./run-docker.ign.sh cargo test --package StoSt --test gtfs --release -- gtfs_with_rt --exact --nocapture --ignored
```

If your GTFS(-RT) files are not located in a `../gtfs/` directory with the same substructure as https://mirror.traines.eu, you may need to manually adjust the paths in the tests.

### Simulation
The simulation component can be used to evaluate the algorithm in repeated runs compared to other algorithms, in particular with respect to how much earlier an actual simulated user may arrive using the StoSt algorithm. In order to run a simulation, the `simulation` binary must be run with a configuration file:

```
./target/release/simulation ./simulation/config/relevant_short_transfers_fuzzy_with_distr.json
```

Or, directly with Cargo and Docker:

```
./run-docker.sh cargo run --release --bin simulation run ./simulation/config/relevant_short_transfers_fuzzy_with_distr.json
```

There are many example configurations in [simulation/config/](https://github.com/traines-source/stochastic-journey-strategies/blob/master/simulation/config/), matching the source GTFS(-RT) data from https://mirror.traines.eu. After a simulation has completed, the results files can be analyzed using:

```
./run-docker.sh cargo run --release --bin simulation analyze ./simulation/runs/*
```

This will print out a variety of statistics about the run, including histograms of certain metrics. For details on the calculation and to alter the output, refer to [src/bin/simulation.rs](https://github.com/traines-source/stochastic-journey-strategies/blob/master/src/bin/simulation.rs). Please get in touch if you want to obtain some example results for analysis.

The simulation can be used to compare against a classical [RAPTOR](https://doi.org/10.1287/trsc.2014.0534) implementation from [nigiri](https://github.com/motis-project/nigiri) and a [CSA MEAT](https://doi.org/10.1145/3274661) implementation in [src/query/csameat.rs](https://github.com/traines-source/stochastic-journey-strategies/blob/master/src/query/csameat.rs) (see the example configs).

# Todo
* Extended walking with a reasonable execution time
* production-ready `provide_timetable` mode (i.e. auto-refresh, parallel querying, etc.)
* ...