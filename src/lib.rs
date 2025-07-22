
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
mod utils;
use utils::*;
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
    pub fn with_hasher(hasher_builder: S) -> Self {
        Hypeerlog {
            hasher: hasher_builder,
            percision: 14,
            registers: vec![0; pow_two(14) as usize],
        }
    }
    /// Creates a new instance with the given Hasher
    /// Silently clamps the percision to 4-18 (inclusive)
    /// Constructs a Hypeerlog with the given percision and Hasher
    /// Silently clamps the percision to 4-20
    pub fn with_hasher_percision(percision: u8, hasher_builder: S) -> Self {
        let p = percision.clamp(4, 20);
        Hypeerlog {
            hasher: hasher_builder,
            percision: p,
            registers: vec![0; pow_two(p) as usize],
        }
    }
}


impl Hypeerlog {
    /// Create a new Hypeerlog
    pub fn new() -> Hypeerlog<Murmur3BuildHasher> {
        Hypeerlog {
            hasher: Murmur3BuildHasher::new(0),
            percision: 14,
            registers: vec![0; pow_two(14) as usize],
        }
    }

    /// Constructs a Hypeerlog with the given percision
    /// Silently clamps the percision to 4-20
    pub fn with_percision(percision: u8) -> Hypeerlog<Murmur3BuildHasher> {
        let p = percision.clamp(4, 20);
        Hypeerlog {
            hasher: Murmur3BuildHasher::new(0),
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
    pub fn estimate_card(&self) -> f64 {
        let m = pow_two(self.percision) as f64;
        let alpha_m = get_alpha_m_bias(m);

        let num_zero_registers = self.registers.iter().filter(|&&val| val == 0).count();

        if num_zero_registers == m as usize {
            return 0.0;
        }

        let harmonic_mean = harmonic_mean(&self.registers);
        let mut estimate = alpha_m * m * m * harmonic_mean;

        // Use LinearCounting if there are still empty buckets AND the raw HLL estimate is low
        if num_zero_registers > 0 && estimate < (2.5 * m) { 
            // Linear Counting formula: m * ln(m / V)
            // V is the number of zero registers.
            estimate = m * (m / num_zero_registers as f64).ln();
        }
        estimate
    }
}









