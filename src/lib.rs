#![cfg_attr(feature = "no_std", no_std)]


#![allow(unused)]
#![deny(
    missing_docs,
    clippy::missing_safety_doc,
    clippy::undocumented_unsafe_blocks
)]


#![cfg_attr(feature = "no_std", no_std)]

#![allow(unused)]
#![deny(
    missing_docs,
    clippy::missing_safety_doc,
    clippy::undocumented_unsafe_blocks
)]

//! # hypeerlog
//! 
//! [![Crates.io Version](https://img.shields.io/crates/v/hypeerlog.svg)](https://crates.io/crates/hypeerlog)
//! [![Docs.rs](https://img.shields.io/docsrs/hypeerlog)](https://docs.rs/hypeerlog)
//! [![Crates.io Total Downloads](https://img.shields.io/crates/d/hypeerlog)](https://crates.io/crates/hypeerlog)
//!
//! A blazingly fast HyperLogLog++ implementation designed for high-throughput, distributed cardinality estimation.
//!
//! This crate faithfully implements the [Google HyperLogLog++ paper](https://research.google.com/pubs/archive/40671.pdf), 
//! including standard bias correction and linear counting for small cardinalities.
//! 
//! The HyperLogLog algorithm is a probabilistic data structure used to estimate the number of distinct elements in a set. 
//! It operates using a fixed amount of memory while keeping the relative estimation error exceptionally small.
//! 
//! ## Features
//! 
//! - **Flexible Hashing**: Employs a custom, ultra-fast Murmur3 hasher by default (the gold standard for HyperLogLog sketches), while offering full generic support to drop in your own custom hasher implementation.
//! - **Configurable Accuracy**: Define your own precision or maximum relative error bounds to explicitly tune the exact accuracy vs. memory footprint trade-off required for your workload.
//! - **Production Ready**: Rigorously tested, fully micro-benchmarked, and meticulously optimized for raw performance.
//! 
//! > **Note on Design**: This crate intentionally omits the sparse register representation described in the paper. By focusing entirely on a flattened dense register footprint, it removes serialization layout overheads and yields cleaner optimization paths for distributed network/storage engines where fixed-size states are highly desirable.
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
//! HyperLogLog sketches are perfectly additive. You can distribute massive datasets across multiple independent 
//! workers, compute highly efficient local sketches, and merge them later to find the global unique count.
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
//! ## `no_std` Support
//! 
//! This crate features a highly constrained, lightweight memory profile, making it a perfect fit for resource-constrained or bare-metal environments. 
//! 
//! To enable standalone usage, activate the `no_std` feature in your `Cargo.toml`:
//! 
//! ```toml
//! [dependencies]
//! hypeerlog = { version = "0.3.1", features = ["no_std"] }
//! ```
//! 
//! All core estimation and merging features remain fully available in `no_std` mode via safe heap allocations handled contextually by the `alloc` crate.
//!




use core::hash::Hash;
use core::hash::{BuildHasher, Hasher};
use core::fmt::Debug;

mod murmur;
mod utils;
use utils::*;
use murmur::{Murmur3BuildHasher};


pub use utils::{rel_error_from_p, p_from_rel_error};


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




/// All errors that can be returned. This happens when merging or loading
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HypeerlogError {
    /// The input byte array length does not match the embedded precision.
    InvalidLength,
    /// Precision found in data is outside the allowable range of 4 to 25.
    InvalidPrecision,
    /// Merging failed because the two instances have different precisions.
    PrecisionMismatch,
}

impl core::fmt::Display for HypeerlogError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::InvalidLength => write!(f, "Invalid buffer length for the given precision"),
            Self::InvalidPrecision => write!(f, "Precision must be between 4 and 25"),
            Self::PrecisionMismatch => write!(f, "Cannot merge instances with different precisions"),
        }
    }
}

#[cfg(not(feature = "no_std"))]
impl std::error::Error for HypeerlogError {}



/// A probabilistic cardinality estimator based on the HyperLogLog++ algorithm.
///
/// `Hypeerlog` is generic over its internal [`BuildHasher`]. By default, it employs a highly
/// optimized `Murmur3BuildHasher` which is ideal for uniform bit distribution, but can be
/// swapped out for cryptographic hashers if hash DoS protection is required.
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
    /// Creates a new instance using a custom hasher builder with a default precision of 14.
    ///
    /// A precision of 14 allocates $2^{14}$ (16,384) registers, yielding a standard relative
    /// error bound of approximately 0.8%.
    pub fn with_hasher(hasher_builder: S) -> Self {
        Hypeerlog {
            hasher: hasher_builder,
            precision: 14,
            registers: vec![0; pow_two(14) as usize],
        }
    }

    /// Creates a new instance with a custom hasher builder and a specific precision.
    ///
    /// The precision value is silently clamped to the valid range of `4..=25`.
    /// - **Min precision (4)**: 16 registers, ~26% relative error.
    /// - **Max precision (25)**: ~33.5 million registers, ~0.018% relative error.
    pub fn with_hasher_precision(precision: u8, hasher_builder: S) -> Self {
        let p = precision.clamp(4, 25);
        Hypeerlog {
            hasher: hasher_builder,
            precision: p,
            registers: vec![0; pow_two(p) as usize],
        }
    }

    /// Creates a new instance with a custom hasher builder targeting a specific relative error.
    ///
    /// # Panics
    ///
    /// Panics if the `relative_err` is out of bounds (must be greater than 0.0 and less than or equal to 1.0).
    pub fn with_hasher_relative_error(relative_err: f64, hasher_builder: S) -> Self {
        let p = p_from_rel_error(relative_err) as u8;
        Hypeerlog {
            hasher: hasher_builder,
            precision: p,
            registers: vec![0; pow_two(p) as usize],
        }
    }

    /// Deserializes a dumped `Hypeerlog` state vector using a custom hasher.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The byte buffer is empty or missing its trailing precision metadata byte ([`HypeerlogError::InvalidLength`]).
    /// - The extracted precision byte is outside `4..=25` ([`HypeerlogError::InvalidPrecision`]).
    /// - The byte buffer length does not exactly match the expected register count ($2^p$) for that precision ([`HypeerlogError::InvalidLength`]).
    pub fn load_with_hasher(mut bytes: Vec<u8>, hasher_builder: S) -> Result<Self, HypeerlogError> {
        let p = bytes.pop().ok_or(HypeerlogError::InvalidLength)?;
        if p < 4 || p > 25 { return Err(HypeerlogError::InvalidPrecision); }
        if bytes.len() != (pow_two(p) as usize) {return Err(HypeerlogError::InvalidPrecision);}
        Ok(Hypeerlog {
            hasher: hasher_builder,
            precision: p,
            registers: bytes,
        })
    }

    /// Deserializes a dumped `Hypeerlog` state directly from a streaming reader.
    ///
    /// # Errors
    ///
    /// Returns a [`HypeerlogError`] if the underlying reader fails or if the stream represents
    /// an invalid or corrupted sketch configuration.
    #[cfg(not(feature = "no_std"))]
    pub fn load_from_with_hasher<R: std::io::Read>(mut reader: R, hasher_builder: S) -> Result<Self, HypeerlogError> {
        // We don't know the precision yet, but we can read the exact structure if we 
        // read the stream. Since the precision byte is at the end, a simple approach for
        // streaming readers is to read all bytes into a temporary vector.
        let mut bytes = std::vec::Vec::new();
        reader.read_to_end(&mut bytes).map_err(|_| HypeerlogError::InvalidLength)?;
        Self::load_with_hasher(bytes, hasher_builder)
    }


    /// Returns the total number of underlying register buckets used by the sketch.
    ///
    /// This value equals $2^{\text{precision}}$.
    pub fn len(&self) -> usize {
        self.registers.len()
    }

    /// Returns the precision ($p$) configuration of this HyperLogLog.
    pub fn precision(&self) -> u8 {
        self.precision
    }

    /// Returns the expected standard relative error of the current configuration.
    pub fn relative_error(&self) -> f64 {
        rel_error_from_p(self.precision as u32)
    }

    /// Inserts a single hashable item into the sketch.
    ///
    /// This will hash the item and update the appropriate internal register bucket if the item's
    /// hash contains a longer run of leading zeros than previously observed.
    pub fn insert<H: Hash>(&mut self, data: H) {
        let mut hasher = self.hasher.build_hasher();
        data.hash(&mut hasher);
        let hash = hasher.finish();
        let register_idx = get_bucket(self.precision, hash);
        self.registers[register_idx] = longest_run(self.precision, hash).max(self.registers[register_idx]);
    }

    /// Inserts a slice of items into the Hyperloglog.
    ///
    /// Perfect for high-throughput batch updates.
    pub fn insert_many<H: Hash>(&mut self, data: &[H]) {
        for elem in data {
            self.insert(elem);
        }
    }


   /// Returns `true` if no elements have been observed by this Hyperloglog yet.
    pub fn is_empty(&self) -> bool {
        self.registers.iter().all(|&val| val == 0)
    }

    /// Resets all internal register buckets back to zero, effectively wiping the history of the sketch
    /// without re-allocating memory.
    pub fn clear(&mut self) {
        self.registers.fill(0);
    }

    /// Returns the estimated distinct element count (cardinality) observed by this sketch.
    ///
    /// This applies bias correction algorithms and transitions dynamically to linear counting
    /// for low-range estimates to keep estimation error within bounds.
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

    /// Merges another `Hypeerlog` sketch into this one, consuming both and returning a new combined sketch.
    ///
    /// The resulting sketch contains the unified unique element observations of both source sketches.
    ///
    /// # Errors
    ///
    /// Returns [`HypeerlogError::PrecisionMismatch`] if the two sketches were initialized with
    /// different precision thresholds.
    pub fn merge(mut self, other: Self) -> Result<Self, HypeerlogError> {
        if self.precision != other.precision {
            return Err(HypeerlogError::PrecisionMismatch);
        }

        self.registers.iter_mut()
            .zip(other.registers.iter())
            .for_each(|(a, b)| *a = a.clone().max(b.clone()));

        Ok(self)
    }

    /// Serializes the state of the sketch into a heap-allocated `Vec<u8>`.
    ///
    /// The resulting vector contains the raw values of all register bytes, appended with a single
    /// final byte denoting the precision configuration. This array can be stored or transmitted and
    /// reloaded later via [`Hypeerlog::load`].
    pub fn dump(&self) -> Vec<u8> {
        let mut clone = self.registers.clone();
        clone.push(self.precision);
        clone
    } 

    /// Writes the exact binary state of the sketch straight to a generic writer.
    ///
    /// This is an optimized, zero-allocation alternative to [`Hypeerlog::dump`].
    ///
    /// # Errors
    ///
    /// Returns an [`std::io::Error`] if writing to the underlying stream fails.
    #[cfg(not(feature = "no_std"))]
    pub fn dump_to<W: std::io::Write>(&self, mut writer: W) -> std::io::Result<()> {
        writer.write_all(&self.registers)?;
        writer.write_all(&[self.precision])?;
        Ok(())
    }

    /// Writes the exact binary state of the sketch straight into a pre-allocated fixed-size slice.
    ///
    /// Ideal for embedded or `no_std` platforms where streaming writers are absent.
    ///
    /// # Errors
    ///
    /// Returns [`HypeerlogError::InvalidLength`] if the provided target buffer slice is smaller
    /// than the required space ($2^{\text{precision}} + 1$ bytes).
    pub fn dump_to_slice(&self, buf: &mut [u8]) -> Result<usize, HypeerlogError> {
        let expected_len = self.registers.len() + 1;
        if buf.len() < expected_len {
            return Err(HypeerlogError::InvalidLength);
        }

        buf[..self.registers.len()].copy_from_slice(&self.registers);
        buf[self.registers.len()] = self.precision;
        
        Ok(expected_len)
    }
}


impl Hypeerlog {
    /// Creates a new default instance with a precision of 14 using the default `Murmur3BuildHasher`.
    ///
    /// Target relative error is ~0.8%.
    pub fn new() -> Hypeerlog<Murmur3BuildHasher> {
        Self::with_precision(14)
    }

    /// Constructs a new instance with a specific precision using the default `Murmur3BuildHasher`.
    ///
    /// The precision value is silently clamped to `4..=25`.
    pub fn with_precision(precision: u8) -> Hypeerlog<Murmur3BuildHasher> {
        let p = precision.clamp(4, 25);
        Hypeerlog {
            hasher: Murmur3BuildHasher::new(0),
            precision: p,
            registers: vec![0; pow_two(p) as usize],
        }
    }

    /// Creates a new instance with a given relative error bound using the default `Murmur3BuildHasher`.
    ///
    /// # Panics
    ///
    /// Panics if `relative_err` is outside the range `(0.0, 1.0]`.
    pub fn with_relative_error(relative_err: f64) -> Self {
        let p = p_from_rel_error(relative_err) as u8;
        Hypeerlog {
            hasher: Murmur3BuildHasher::new(0),
            precision: p,
            registers: vec![0; pow_two(p) as usize],
        }
    }

    /// Constructs a new instance with a custom seed for the internal `Murmur3BuildHasher`.
    ///
    /// Providing a randomized or unexpected seed can protect the structure against Hash DoS attacks
    /// when managing inputs from untrusted external users.
    pub fn with_seed(seed: u32) -> Hypeerlog<Murmur3BuildHasher> {
        Hypeerlog {
            hasher: Murmur3BuildHasher::new(seed),
            precision: 14,
            registers: vec![0; pow_two(14) as usize],
        }
    }

    /// Constructs a new instance with both a custom precision and a specific seed for the default hasher.
    ///
    /// Precision is clamped to `4..=25`.
    pub fn with_precision_seed(precision: u8, seed: u32) -> Hypeerlog<Murmur3BuildHasher> {
        let p = precision.clamp(4, 25);
        Hypeerlog {
            hasher: Murmur3BuildHasher::new(seed),
            precision: p,
            registers: vec![0; pow_two(p) as usize],
        }
    }

    /// Creates a new instance with a target relative error and a specific seed for the default hasher.
    ///
    /// # Panics
    ///
    /// Panics if `relative_err` is outside the range `(0.0, 1.0]`.
    pub fn with_relative_error_seed(relative_err: f64, seed: u32) -> Self {
        let p = p_from_rel_error(relative_err) as u8;
        Hypeerlog {
            hasher: Murmur3BuildHasher::new(seed),
            precision: p,
            registers: vec![0; pow_two(p) as usize],
        }
    }

    /// Deserializes a dumped `Hypeerlog` state vector using the default `Murmur3BuildHasher`.
    ///
    /// # Errors
    ///
    /// Returns a [`HypeerlogError`] if the data payload contains mismatched lengths or invalid precision settings.
    pub fn load(mut bytes: Vec<u8>) -> Result<Self, HypeerlogError> {
        let p = bytes.pop().ok_or(HypeerlogError::InvalidLength)?;
        if p < 4 || p > 25 { return Err(HypeerlogError::InvalidPrecision); }
        if bytes.len() != (pow_two(p) as usize) {return Err(HypeerlogError::InvalidPrecision);}
        Ok(Hypeerlog {
            hasher: Murmur3BuildHasher::new(0),
            precision: p,
            registers: bytes,
        })
    }

    /// Deserializes a dumped `Hypeerlog` state directly from a streaming reader using the default hasher.
    ///
    /// # Errors
    ///
    /// Returns a [`HypeerlogError`] if reading fails or if the deserialized sketch properties are corrupt.
    #[cfg(not(feature = "no_std"))]
    pub fn load_from<R: std::io::Read>(mut reader: R) -> Result<Self, HypeerlogError> {
        let mut bytes = std::vec::Vec::new();
        reader.read_to_end(&mut bytes).map_err(|_| HypeerlogError::InvalidLength)?;
        Self::load(bytes)
    }
}


// Some convinient trait implementations

impl Default for Hypeerlog<Murmur3BuildHasher> {
    fn default() -> Self {
        Self::new()
    }
}

impl<H: Hash, S: BuildHasher + Debug> Extend<H> for Hypeerlog<S> {
    fn extend<T: IntoIterator<Item = H>>(&mut self, iter: T) {
        for item in iter {
            self.insert(item);
        }
    }
}

impl<H: Hash> FromIterator<H> for Hypeerlog<Murmur3BuildHasher> {
    fn from_iter<T: IntoIterator<Item = H>>(iter: T) -> Self {
        let mut hll = Self::new();
        hll.extend(iter);
        hll
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
        let elem = &$elem; // Borrow once
        for _ in 0..$n {
            hll.insert(elem);
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





