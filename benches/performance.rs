
use criterion::{criterion_group, criterion_main, Criterion};
use std::hint::black_box;
use rand::prelude::*;
use std::collections::HashSet;
use std::hash::Hash;
use hypeerlog::Hypeerlog;


fn generate_random_list_with_cardinality(length: usize, cardinality: usize) -> Result<Vec<u64>, String> {
    if cardinality > length {
        return Err("Cardinality cannot be greater than length.".to_string());
    }
    if length == 0 {
        return Ok(vec![]);
    }
    let mut rng = rand::rng();
    let mut unique_elems = HashSet::with_capacity(cardinality);
    let mut result_list = Vec::with_capacity(length);
    // Generate 'cardinality' unique elements
    for _ in 0..cardinality {
        loop {
            let num = rng.random::<u64>();
            if unique_elems.insert(num) { // Try to insert into the set; returns true if new
                result_list.push(num);
                break;
            }
            // If `num` was already in the set, loop again to generate another number
        }
    }
    let unique_elems_vec: Vec<u64> = unique_elems.into_iter().collect();
    for _ in 0..(length - cardinality) {
        let random_index = rand::random_range(0..=unique_elems_vec.len() - 1);
        result_list.push(unique_elems_vec[random_index]);
    }
    result_list.shuffle(&mut rng);
    Ok(result_list)
}



// Takes the true cardinality and the elements, and returns the estimated cardinality and the relative error
fn run_trial<H: Hash>(p: u8, card: usize, elems: &[H]) -> (f64, f64) {
    let mut hll = Hypeerlog::with_percision(p);
    hll.batch_add(elems);
    let estimated_cardinality = hll.estimate_card();
    let relative_error = (estimated_cardinality as f64 - card as f64).abs() / card as f64;
    (estimated_cardinality, relative_error)
}


const P_VALUES: [u8; 4] = [6, 8, 10, 12];
const CARDS: [u32; 4] = [700, 800, 900, 1000];



fn bench_hll_combinations(c: &mut Criterion) {
    let mut group = c.benchmark_group("HLL_Performance");

    group.sample_size(30);
    group.warm_up_time(std::time::Duration::from_secs(1));

    for p in P_VALUES {
        for &card in CARDS.iter() {

            // Generate a unique ID for each specific benchmark combination
            let bench_id = format!("p={}_card={}", p, card);

            group.bench_function(bench_id, move |b| {
                // SETUP PHASE: We generate the list of elements here so that list generation time
                // is NOT included in the benchmarked time
                let list = generate_random_list_with_cardinality(10_000, card as usize)
                           .expect("Failed to generate list for benchmark");

                // BENCHMARK PHASE:
                b.iter(|| {
                    let (_, _) = black_box(run_trial(p, card as usize, &list));
                });
            });
        }
    }
    group.finish();
}









//--------------
// Running the benchmarks
//--------------


criterion_group!(benches, bench_hll_combinations);
criterion_main!(benches);
