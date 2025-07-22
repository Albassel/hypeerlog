
#![allow(unused)]
#![deny(
    //missing_docs,
    clippy::missing_safety_doc,
    clippy::undocumented_unsafe_blocks
)]


use core::hash::Hash;
use std::hash::{BuildHasher, Hasher};
use std::fmt::Debug;

mod murmur;
use murmur::{Murmur3BuildHasher};


/// A struct implementing HyperLogLog that is generic over the Hasher
#[derive(Debug, PartialEq, Eq)]
pub struct Hypeerlog<S = Murmur3BuildHasher> 
where
    S: BuildHasher + Debug,
{
    hasher: S,
    percision: u8,
    registers: Vec<u8>,
}


impl<S> Hypeerlog<S>
where
    S: BuildHasher + Debug,
{
    /// Creates a new instance with the given Hasher
    /// Silently clamps the percision to 4-18 (inclusive)
    pub fn new_with_hasher(percision: Option<u8>, hasher_builder: S) -> Self {
        let p = percision.unwrap_or(8);
        Hypeerlog {
            hasher: hasher_builder,
            percision: p,
            registers: vec![0; pow_two(p) as usize],
        }
    }
}


impl Hypeerlog {
        /// Silently clamps the percision to 4-18 (inclusive)
    /// You may want to use a random seed to prevent hash DoS attacks
    pub fn new(percision: Option<u8>, seed: Option<u32>) -> Hypeerlog<Murmur3BuildHasher> {
        let p = percision.unwrap_or(8).clamp(4, 18);
        let s = seed.unwrap_or(0);
        Hypeerlog {
            hasher: Murmur3BuildHasher::new(s),
            percision: p,
            registers: vec![0; pow_two(p) as usize],
        }
    }

    /// Adds data to this Hyperloglog to count the cardinality
    pub fn add<H: Hash>(&mut self, data: H) {
        let mut hasher = self.hasher.build_hasher();
        data.hash(&mut hasher);
        let hash = hasher.finish();
        let register_idx = get_bucket(self.percision, hash);
        self.registers[register_idx] = longest_run(self.percision, hash).max(self.registers[register_idx]);
    }

    /// Adds a whole slice of data to this Hyperloglog to count the cardinality
    pub fn batch_add<H: Hash>(&mut self, data: &[H]) {
        for elem in data {
            self.add(elem);
        }
    }

    /// Returns the estimated cardinality for the values added so far
    /// Returns f64::INFINITY if all the registers are 0 (for example, when no data is added to the Hypeerlog)
    pub fn estimate_card(&self) -> f64 {
        let m = pow_two(self.percision) as f64;
        let alpha_m = get_alpha_m_constant(m);

        let num_zero_registers = self.registers.iter().filter(|&&val| val == 0).count();

        // 1. Handle case where all registers are zero (0 elements added)
        if num_zero_registers == m as usize {
            return 0.0;
        }

        // 2. Calculate the raw HyperLogLog estimate
        let harmonic_mean_result = harmonic_mean(&self.registers);
        let mut estimate = alpha_m * m * m * harmonic_mean_result;

        // Use LinearCounting if there are still empty buckets AND the raw HLL estimate is low
        // 2.5 * m is a common threshold
        if num_zero_registers > 0 && estimate < (2.5 * m) { 
            // Linear Counting formula: m * ln(m / V)
            // V is the number of zero registers.
            estimate = m * (m / num_zero_registers as f64).ln();
        }

        // 4. Large Range Correction 
        // This correction is for when the estimate is very large, approaching the limits
        // of the hash space. For 64-bit hashes, it's often not necessary unless you're counting truly massive cardinalities (e.g., > 10^10)
        const TWO_POW_64_OVER_30: f64 = (1u64 << 63) as f64 / 15.0; // Approximation for 2^64 / 30
        if estimate > TWO_POW_64_OVER_30 {
            estimate = -TWO_POW_64_OVER_30 * (1.0 - estimate / TWO_POW_64_OVER_30).ln();
        }

        estimate
    }
}




// max value that can be passed is 31, which is not a big problem because this is way beyound the supported max percision
fn pow_two(p: u8) -> u32 {
    return (1 << p) as u32
}

fn get_bucket(precision: u8, hash: u64) -> usize {
    let mask: u64 = match precision {
        0 => 0, // No bits selected, so mask is 0. All hashes map to bucket 0.
        1..=63 => (1u64 << precision) - 1, // Creates a mask with 'precision' number of 1s
        _ => panic!("Invalid percision used"),
    };
    (mask & hash) as usize
}

fn longest_run(percision: u8, hash: u64) -> u8 {
    (hash >> percision).trailing_zeros() as u8 + 1
}


fn harmonic_mean(registers: &[u8]) -> f64 {
    let sum: f64 = registers.iter().map(|&val| 2.0f64.powi(-(val as i32))).sum();
    if sum == 0.0 {
        f64::INFINITY
    } else {
        1.0 / sum
    }
}

// Bias correction
fn get_alpha_m_constant(m: f64) -> f64 {
        match m {
            4.0 => 0.673, // for m = 16
            5.0 => 0.697, // for m = 32
            6.0 => 0.709, // for m = 64
            _ => 0.7213 / (1.0 + 1.079 / m),
        }
    }




