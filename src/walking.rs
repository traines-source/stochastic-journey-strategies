use motis_nigiri::Footpath;

use crate::connection::Station;


const WALKING_METRES_PER_SECOND: f32 = 1.5;
const MAX_WALKING_METRES: f32 = 5000.0;

fn geodist_meters(stop1: &Station, stop2: &Station) -> f32 {       
    let r = 6371e3;
    let x = (stop2.lon.to_radians()-stop1.lon.to_radians()) * ((stop1.lat.to_radians()+stop2.lat.to_radians())/2 as f32).cos();
    let y = stop2.lat.to_radians()-stop1.lat.to_radians();
    (x*x + y*y).sqrt() * r
}

pub fn shorten_footpaths(stations: &mut Vec<Station>) {
    for i in 0..stations.len() {
        for j in 0..stations[i].footpaths.len() {
            let dur = (geodist_meters(&stations[i], &stations[stations[i].footpaths[j].target_location_idx])/WALKING_METRES_PER_SECOND/60.0).round() as u16;
            stations[i].footpaths[j].duration = std::cmp::min(std::cmp::max(dur, 1), stations[i].footpaths[j].duration);
        }
        stations[i].transfer_time = 1;
    }
}

pub fn create_quadratic_footpaths(stations: &mut Vec<Station>) {
	let mut ctr = 0;
	for i in 0..stations.len() {
        for j in 0..stations.len() {
			let dist = geodist_meters(&stations[i], &stations[j]);
			if dist < MAX_WALKING_METRES {
				let dur = (dist/WALKING_METRES_PER_SECOND/60.0).round() as u16;
				stations[i].footpaths.push(Footpath{
					target_location_idx: j,
					duration: dur
				});
				ctr += 1;
			}            
        }
        stations[i].transfer_time = 1;
    }
	println!("Created {} footpaths", ctr);
}