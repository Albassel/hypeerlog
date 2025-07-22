
#![allow(unused)]
#![deny(
    missing_docs,
    clippy::missing_safety_doc,
    clippy::undocumented_unsafe_blocks
)]

//! # htpp
//!
//! A library for parsing HTTP requests and responses. The focus is on speed and safety. It is intentionally strict
//! to minimize HTTP attacks. It can also parse URLs
//! 
//! ## Working with [Request]
//! 
//! You can parse a request as follows:
//! 
//! ```rust
//! use htpp::{Request, EMPTY_HEADER};
//! 
//! let req = b"GET /index.html HTTP/1.1\r\n\r\n";
//! let mut headers = [EMPTY_HEADER; 10];
//! let parsed = Request::parse(req, &mut headers).unwrap();
//! assert!(parsed.method == htpp::Method::Get);
//! assert!(parsed.path == "/index.html");
//! ```
//! You can create a request as follows:
//! 
//! ```rust
//! use htpp::{Method, Request, Header};
//! 
//! let method = Method::Get;
//! let path = "/index.html";
//! let mut headers = [Header::new("Accept", b"*/*")];
//! let req = Request::new(method, path, &headers, b"");
//! ```
//! ## Working with [Response]
//! 
//! You can parse a response as follows:
//! 
//! ```rust
//! use htpp::{Response, EMPTY_HEADER};
//! 
//! let req = b"HTTP/1.1 200 OK\r\n\r\n";
//! let mut headers = [EMPTY_HEADER; 10];
//! let parsed = Response::parse(req, &mut headers).unwrap();
//! assert!(parsed.status == 200);
//! assert!(parsed.reason == "OK");
//! ```
//! 
//! You can create a response as follows:
//! 
//! ```rust
//! use htpp::{Response, Header};
//! 
//! let status = 200;
//! let reason = "OK";
//! let mut headers = [Header::new("Connection", b"keep-alive")];
//! let req = Response::new(status, reason, &mut headers, b"");
//! ```
//! 
//! After parsing a request, you can also parse the path part of the request inclusing query parameters as follows:
//! 
//! ```rust
//! use htpp::{Request, EMPTY_QUERY, Url, EMPTY_HEADER};
//! 
//! let req = b"GET /index.html?query1=value&query2=value HTTP/1.1\r\n\r\n";
//! let mut headers = [EMPTY_HEADER; 10];
//! let parsed_req = Request::parse(req, &mut headers).unwrap();
//! let mut queries_buf = [EMPTY_QUERY; 10];
//! let url = Url::parse(parsed_req.path.as_bytes(), &mut queries_buf).unwrap();
//! assert!(url.path == "/index.html");
//! assert!(url.query_params.unwrap()[0].name == "query1");
//! assert!(url.query_params.unwrap()[0].val == "value");
//! ```
//! 



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
        harmonic_mean(&self.registers)
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
    (hash >> percision).trailing_zeros() as u8
}


fn harmonic_mean(registers: &[u8]) -> f64 {
    let sum: f64 = registers.iter().map(|&val| 2.0f64.powi(-(val as i32))).sum();
    if sum == 0.0 {
        f64::INFINITY
    } else {
        1.0 / sum
    }
}





