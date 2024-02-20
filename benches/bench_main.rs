#[macro_use]
extern crate rmp_serde as rmps;

mod basic;

use criterion::criterion_main;

criterion_main!(basic::benches);