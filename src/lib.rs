#![cfg_attr(feature = "no_std", no_std)]


#![allow(unused)]
#![deny(
    missing_docs,
    clippy::missing_safety_doc,
    clippy::undocumented_unsafe_blocks
)]


//! # hypeerlog
//!
//! A blazingly fast HyperLogLog++ implementation designed for high-throughput, distributed cardinality estimation.
//!
//! This crate faithfully implements the Google [HyperLogLog++ paper](https://research.google.com/pubs/archive/40671.pdf), 
//! including sparse/dense representation switching and standard bias correction.
//!
//! ## Estimating Cardinality
//!
//! ```rust
//! use hypeerlog::Hypeerlog;
//!
//! let elems = vec![1, 2, 3, 4, 5, 6, 7, 1, 1, 2];
//!
//! let mut hll = Hypeerlog::new();
//! hll.insert_many(&elems);
//!
//! // The estimation is guaranteed to be within the typical HLL error bounds (e.g., ~2%).
//! assert_eq!(hll.cardinality().floor(), 7.0); 
//! ```
//!
//! ## Distributed Workloads & Merging
//!
//! HyperLogLog sketches are additive. You can distribute a massive dataset across multiple 
//! workers, compute local sketches, and merge them later to find the total unique count.
//!
//! ```rust
//! use hypeerlog::Hypeerlog;
//!
//! let elems = vec![1, 2, 3, 4, 5, 6, 7, 1, 1, 2];
//!
//! let mut hll_one = Hypeerlog::new();
//! hll_one.insert_many(&elems[0..5]);
//!
//! let mut hll_two = Hypeerlog::new();
//! hll_two.insert_many(&elems[5..]);
//!
//! // Merge the second sketch into the first
//! let merged = hll_one.merge(hll_two).unwrap();
//! assert_eq!(merged.cardinality().floor(), 7.0);
//! ```
//!
//! ## `no_std` support
//! 
//! This crate provides `no_std` support using the no_std feature:
//! 
//! ```toml
//! [dependencies]
//! hypeerlog = { version = "0.3.1", features = ["no_std"] }
//! ```
//! 
//! This allows you to use a lighweight performant HyperLogLog++ with minimal memory footprint in embedded environments.
//! 




use core::hash::Hash;
use core::hash::{BuildHasher, Hasher};
use core::fmt::Debug;

mod murmur;
mod utils;
use utils::*;
use murmur::{Murmur3BuildHasher};


pub use utils::rel_error_from_p;


// Handle vector allocation contextually
#[cfg(feature = "no_std")]
extern crate alloc;

#[cfg(feature = "no_std")]
use alloc::vec::Vec;
#[cfg(feature = "no_std")]
use alloc::vec;

#[cfg(not(feature = "no_std"))]
use std::vec::Vec;
#[cfg(not(feature = "no_std"))]
use std::vec;


/// A struct implementing HyperLogLog that is generic over the Hasher
#[derive(Debug, PartialEq, Eq)]
pub struct Hypeerlog<S = Murmur3BuildHasher> 
where
    S: BuildHasher + Debug,
{
    hasher: S,
    precision: u8,
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
            precision: 14,
            registers: vec![0; pow_two(14) as usize],
        }
    }

    /// Creates a new instance with the given Hasher and precision
    /// Silently clamps the precision to 4-25, corresponding to a relative error of 26% all the way to 0.018% (the cardinality 
    /// you will get will be at most this far off from the true cardinality)
    pub fn with_hasher_precision(precision: u8, hasher_builder: S) -> Self {
        let p = precision.clamp(4, 25);
        Hypeerlog {
            hasher: hasher_builder,
            precision: p,
            registers: vec![0; pow_two(p) as usize],
        }
    }

    /// Creates a new instance with the given Hasher and relative error
    /// Panics if the relative error passed is <0 or >1
    pub fn with_hasher_relative_error(relative_err: f64, hasher_builder: S) -> Self {
        let p = p_from_rel_error(relative_err) as u8;
        Hypeerlog {
            hasher: hasher_builder,
            precision: p,
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
            precision: p.unwrap(),
            registers: bytes,
        })
    }


    /// The number of registeres used internally
    pub fn len(&self) -> usize {
        self.registers.len()
    }

    /// Inserts data to this Hyperloglog to count the cardinality
    pub fn insert<H: Hash>(&mut self, data: H) {
        let mut hasher = self.hasher.build_hasher();
        data.hash(&mut hasher);
        let hash = hasher.finish();
        let register_idx = get_bucket(self.precision, hash);
        self.registers[register_idx] = longest_run(self.precision, hash).max(self.registers[register_idx]);
    }

    /// Inserts a whole slice of data to this Hyperloglog to count the cardinality
    pub fn insert_many<H: Hash>(&mut self, data: &[H]) {
        for elem in data {
            self.insert(elem);
        }
    }


    /// Checks whether the hll is empty (i,e there were no data inserted)
    pub fn is_empty(&self) -> bool {
        self.registers.iter().all(|&val| val == 0)
    }

    /// Clears all data inserted into the hll
    pub fn clear(&mut self) {
        self.registers.iter_mut().for_each(|r| *r = 0)
    }


    /// Returns the estimated cardinality for the values added so far
    pub fn cardinality(&self) -> f64 {
        let m = pow_two(self.precision) as f64;
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
            // estimate = m * (m / num_zero_registers as f64).ln();

            let ratio = m / num_zero_registers as f64;
            
            #[cfg(not(feature = "no_std"))]
            {
                estimate = m * ratio.ln();
            }
            #[cfg(feature = "no_std")]
            {
                estimate = m * libm::log(ratio); // libm::log is the natural log (ln)
            }
        }
        estimate
    }

    /// Merges 2 HyperLogLogs, returning the merged hll
    /// The precision of the 2 hll must be the same or an error is returned
    /// The 2 hll can use different hashers, but the hasher used for the merged hll is that of the first
    pub fn merge(mut self, other: Self) -> Result<Self, ()> {
        if self.precision != other.precision {
            return Err(());
        }

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
        clone.push(self.precision);
        clone
    }
}


impl Hypeerlog {
    /// Create a new hll with a precision of 14, corresponding to a relative error of 0.8%, (the cardinality 
    /// you will get will be at most this far off from the true cardinality, which is sufficient for most cases)
    pub fn new() -> Hypeerlog<Murmur3BuildHasher> {
        Self::with_precision(14)
    }

    /// Constructs a hll with the given precision
    /// Silently clamps the precision to 4-25, corresponding to a relative error of 26% all the way to 0.018% (the cardinality 
    /// you will get will be at most this far off from the true cardinality)
    pub fn with_precision(precision: u8) -> Hypeerlog<Murmur3BuildHasher> {
        let p = precision.clamp(4, 25);
        Hypeerlog {
            hasher: Murmur3BuildHasher::new(0),
            precision: p,
            registers: vec![0; pow_two(p) as usize],
        }
    }

    /// Creates a new instance with the given relative error
    /// Panics if the relative error passed is <0 or >1
    pub fn with_relative_error(relative_err: f64) -> Self {
        let p = p_from_rel_error(relative_err) as u8;
        Hypeerlog {
            hasher: Murmur3BuildHasher::new(0),
            precision: p,
            registers: vec![0; pow_two(p) as usize],
        }
    }

    /// Constructs a new Hypeerlog with an internal hasher with the given seed
    /// This can be useful when exposing the hll to outside users to prevent hash DoS
    /// When constructing a new hll using this function, make sure to use a seed with an unexpected value
    pub fn with_seed(seed: u32) -> Hypeerlog<Murmur3BuildHasher> {
        Hypeerlog {
            hasher: Murmur3BuildHasher::new(seed),
            precision: 14,
            registers: vec![0; pow_two(14) as usize],
        }
    }

    /// Constructs a hll with the given precision and seed for the internal hasher
    /// Silently clamps the precision to 4-25, corresponding to a relative error of 26% all the way to 0.018% (the cardinality 
    /// you will get will be at most this far off from the true cardinality)
    /// This can be useful when exposing the hll to outside users to prevent hash DoS attacks
    pub fn with_precision_seed(precision: u8, seed: u32) -> Hypeerlog<Murmur3BuildHasher> {
        let p = precision.clamp(4, 25);
        Hypeerlog {
            hasher: Murmur3BuildHasher::new(seed),
            precision: p,
            registers: vec![0; pow_two(p) as usize],
        }
    }

    /// Creates a new instance with the given relative error and seed
    /// Panics if the relative error passed is <0 or >1
    /// This can be useful when exposing the hll to outside users to prevent hash DoS attacks
    pub fn with_relative_error_seed(relative_err: f64, seed: u32) -> Self {
        let p = p_from_rel_error(relative_err) as u8;
        Hypeerlog {
            hasher: Murmur3BuildHasher::new(seed),
            precision: p,
            registers: vec![0; pow_two(p) as usize],
        }
    }

    /// Reloads a dumped hll with the default hasher
    /// Returns an error when the bytes passed are not a valud hll
    pub fn load(mut bytes: Vec<u8>) -> Result<Self, ()> {
        let p = bytes.pop().ok_or(())?;
        if p < 4 || p > 25 { return Err(()); }
        if bytes.len() != (pow_two(p) as usize) {return Err(());}
        Ok(Hypeerlog {
            hasher: Murmur3BuildHasher::new(0),
            precision: p,
            registers: bytes,
        })
    }
}



/// A convinient macro to create a Hypeerlog by directly adding elements to it, similar to the vec! macro
#[macro_export]
macro_rules! hll {
    // Matches: hll![] or hll!()
    () => {
        $crate::Hypeerlog::new()
    };
    
    // Matches: hll![elem; n] 
    // Inserts the element `n` times into a default Hypeerlog instance
    // This is not very useful since the cardinality would still be one but is added for completeness
    ($elem:expr; $n:expr) => {{
        let mut hll = $crate::Hypeerlog::new();
        let elem = $elem;
        for _ in 0..$n {
            hll.insert(&elem);
        }
        hll
    }};

    // Matches: hll![1, 2, 3] or hll![1, 2, 3,]
    ($($x:expr),+ $(,)?) => {{
        let mut hll = $crate::Hypeerlog::new();
        $(
            hll.insert($x);
        )+
        hll
    }};
}





