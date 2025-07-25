
#![allow(unused)]
#![deny(
    missing_docs,
    clippy::missing_safety_doc,
    clippy::undocumented_unsafe_blocks
)]


//! # hypeerlog
//!
//! A blazingly fast HyperLogLog implementation that can be distributed across multiple devices
//! 
//! This implementes all optimizations in the Google paper (except sparse, which is planned for later):  https://research.google.com/pubs/archive/40671.pdf
//! 
//! ## Estimating cardinality
//! 
//! ```rust
//! use hypeerlog::Hypeerlog;
//! 
//! let elems = vec![1, 2, 3, 4, 5, 6, 7, 1, 1, 2];
//! 
//! let mut hll = Hypeerlog::new();
//! hll.insert_many(&elems);
//! hll.insert_many(&elems);
//! 
//! // Should be within 2% of the real cardinality
//! hll.cardinality();
//! ```
//! 
//! ## Distributing the work
//! 
//! You can divide the dataset onto multiple computers, dump the hll when you finish adding the data, load the dump into another computer, merge all the hll, and then calculate the cardinality of the merged hll to get the cardinality for the whole dataset:
//! 
//! 
//! ```rust
//! use hypeerlog::Hypeerlog;
//! 
//! let elems = vec![1, 2, 3, 4, 5, 6, 7, 1, 1, 2];
//! 
//! let mut hll_one = Hypeerlog::new();
//! hll_one.insert_many(&elems[0..5]);
//! hll_one.insert_many(&elems[0..5]);
//! 
//! let mut hll_two = Hypeerlog::new();
//! hll_two.insert_many(&elems[5..]);
//! hll_two.insert_many(&elems[5..]);
//! 
//! hll_one.merge(hll_two).unwrap().cardinality();
//! hll_one.merge(hll_two).unwrap().cardinality();
//! ```
//! 




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
    pub fn with_hasher(hasher_builder: S) -> Self {
        Hypeerlog {
            hasher: hasher_builder,
            percision: 14,
            registers: vec![0; pow_two(14) as usize],
        }
    }
    /// Creates a new instance with the given Hasher and percision
    /// Silently clamps the percision to 4-25
    pub fn with_hasher_percision(percision: u8, hasher_builder: S) -> Self {
        let p = percision.clamp(4, 25);
        Hypeerlog {
            hasher: hasher_builder,
            percision: p,
            registers: vec![0; pow_two(p) as usize],
        }
    }

    /// Reloads a dumped hll with the given hasher
    /// Returns an error when the bytes passed are not a valud hll
    pub fn load_with_hasher(mut bytes: Vec<u8>, hasher_builder: S) -> Result<Self, ()> {
        let p = bytes.pop();
        if p.is_none() {return Err(());}
        if bytes.len() != (pow_two(p.unwrap()) as usize) {return Err(());}
        Ok(Hypeerlog {
            hasher: hasher_builder,
            percision: p.unwrap(),
            registers: bytes,
        })
    }

    /// Reloads a dumped hll with the given hasher
    /// Returns an error when the bytes passed are not a valud hll
    pub fn load_with_hasher(mut bytes: Vec<u8>, hasher_builder: S) -> Result<Self, ()> {
        let p = bytes.pop();
        if p.is_none() {return Err(());}
        if bytes.len() != (pow_two(p.unwrap()) as usize) {return Err(());}
        Ok(Hypeerlog {
            hasher: hasher_builder,
            percision: p.unwrap(),
            registers: bytes,
        })
    }
}


impl Hypeerlog {
    /// Create a new hll with a percision of 14 (sufficient for most cases)
    pub fn new() -> Hypeerlog<Murmur3BuildHasher> {
        Self::with_percision(14)
        Self::with_percision(14)
    }

    /// Constructs a hll with the given percision
    /// Silently clamps the percision to 4-20
    pub fn with_percision(percision: u8) -> Hypeerlog<Murmur3BuildHasher> {
        let p = percision.clamp(4, 20);
        Hypeerlog {
            hasher: Murmur3BuildHasher::new(0),
            percision: p,
            registers: vec![0; pow_two(p) as usize],
        }
    }

    /// Constructs a new Hypeerlog with an internal hasher with the given seed
    /// This can be useful when exposing the hll to outside users to prevent hash DoS
    /// When constructing a new hll using this function, make sure to use a seed with an unexpected value
    pub fn with_seed(seed: u32) -> Hypeerlog<Murmur3BuildHasher> {
        Hypeerlog {
            hasher: Murmur3BuildHasher::new(seed),
            percision: 14,
            registers: vec![0; pow_two(14) as usize],
        }
    }

    /// Constructs a hll with the given percision and seed for the internal hasher
    /// Silently clamps the percision to 4-20
    /// This can be useful when exposing the hll to outside users to prevent hash DoS
    /// When constructing a new hll using this function, make sure to use a seed with an unexpected value
    pub fn with_percision_seed(percision: u8, seed: u32) -> Hypeerlog<Murmur3BuildHasher> {
        let p = percision.clamp(4, 20);
        Hypeerlog {
            hasher: Murmur3BuildHasher::new(seed),
            percision: p,
            registers: vec![0; pow_two(p) as usize],
        }
    }

    /// The number of registeres used internally
    pub fn registers(&self) -> usize {
        self.registers.len()
    }

    /// Inserts data to this Hyperloglog to count the cardinality
    pub fn insert<H: Hash>(&mut self, data: H) {
    /// Inserts data to this Hyperloglog to count the cardinality
    pub fn insert<H: Hash>(&mut self, data: H) {
        let mut hasher = self.hasher.build_hasher();
        data.hash(&mut hasher);
        let hash = hasher.finish();
        let register_idx = get_bucket(self.percision, hash);
        self.registers[register_idx] = longest_run(self.percision, hash).max(self.registers[register_idx]);
    }

    /// Inserts a whole slice of data to this Hyperloglog to count the cardinality
    pub fn insert_many<H: Hash>(&mut self, data: &[H]) {
    /// Inserts a whole slice of data to this Hyperloglog to count the cardinality
    pub fn insert_many<H: Hash>(&mut self, data: &[H]) {
        for elem in data {
            self.insert(elem);
            self.insert(elem);
        }
    }


    /// Checks whether the hll is empty (i,e there were no data inserted)
    pub fn is_empty<H: Hash>(&self) -> bool {
        self.registers.iter().all(|&val| val == 0)
    }

    /// Clears all data inserted into the hll
    pub fn clear<H: Hash>(&mut self) {
        self.registers.iter_mut().for_each(|r| *r = 0)
    }


    /// Returns the estimated cardinality for the values added so far
    pub fn cardinality(&self) -> f64 {
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

    /// Merges 2 HyperLogLogs, returning the merged hll
    /// The percision of the 2 hll must be the same or an error is returned
    /// The 2 hll can use different hashers, but the hasher used for the merged hll is that of the first
    pub fn merge(mut self, other: Self) -> Result<Self, ()> {
        if self.percision != other.percision {
            return Err(());
        }

        self.registers.iter_mut()
            .zip(other.registers.iter())
            .for_each(|(a, b)| *a = a.clone().max(b.clone()));

        Ok(self)
        self.registers.iter_mut()
            .zip(other.registers.iter())
            .for_each(|(a, b)| *a = a.clone().max(b.clone()));

        Ok(self)
    }

    /// Returns a Vec<u8> representing the internal state of the hll
    /// You can then load that dump and continue from where you started
    /// This can be useful for distributing the computation over many devices, 
    /// for example, by writing the dump to a file, loading the dump on another 
    /// device, and merging the hll
    pub fn dump(&self) -> Vec<u8> {
        let mut clone = self.registers.clone();
        clone.push(self.percision);
        clone
    }

    /// Reloads a dumped hll with the default hasher
    /// Returns an error when the bytes passed are not a valud hll
    pub fn load(mut bytes: Vec<u8>) -> Result<Self, ()> {
        let p = bytes.pop();
        if p.is_none() {return Err(());}
        if bytes.len() != (pow_two(p.unwrap()) as usize) {return Err(());}
        Ok(Hypeerlog {
            hasher: Murmur3BuildHasher::new(0),
            percision: p.unwrap(),
            registers: bytes,
        })
    }
}









