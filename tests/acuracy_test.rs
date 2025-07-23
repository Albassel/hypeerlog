use hypeerlog::Hypeerlog;
use rand::prelude::*;
use std::collections::HashSet;
use std::hash::Hash;



#[test]
fn test_accuracy() {
    let p_values = vec![6, 8, 10, 12];
    let cardinalities = vec![100, 200, 300, 400, 500, 600, 700, 800, 900, 1000];

    println!("p, m, estimate, true_cardinality, relative_Error");

    for p in p_values {
        let m = 1_usize << p;
        for true_card in &cardinalities {
            let list = generate_random_list_with_cardinality(3000, *true_card).unwrap();
            let (estimate, relative_err) = run_trial(p, *true_card, &list);

            println!("{}, {}, {}, {}, {:.2}", p, m, estimate, true_card, relative_err);
        }
    }
}




#[test]
fn test_dump_reload() {
    let list = generate_random_list_with_cardinality(10_000, 1000);
    let mut hll = Hypeerlog::new();
    hll.insert_many(&list.unwrap());
    let dumped = hll.dump();
    assert!(Hypeerlog::load(dumped).unwrap() == hll);
}




#[test]
fn test_merge() {
    let list_one = generate_random_list_with_cardinality(10_000, 1000);
    let list_two = generate_random_list_with_cardinality(10_000, 1000);

    let mut hll_one = Hypeerlog::new();
    hll_one.insert_many(list_one.as_ref().unwrap());

    let mut hll_two = Hypeerlog::new();
    hll_two.insert_many(list_two.as_ref().unwrap());

    let mut hll_three = Hypeerlog::new();
    hll_three.insert_many(&list_one.unwrap());
    hll_three.insert_many(&list_two.unwrap());

    let merged_card = hll_one.merge(hll_two).unwrap().cardinality();
    let card = hll_three.cardinality();

    assert!(merged_card == card);
}




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

    hll.insert_many(elems);

    let estimated_cardinality = hll.cardinality();
    let relative_error = (estimated_cardinality as f64 - card as f64).abs() / card as f64;

    (estimated_cardinality, relative_error)
}


