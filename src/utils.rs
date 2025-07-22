
// max value that can be passed is 31, which is not a big problem because this is way beyound the supported max percision
#[inline]
pub fn pow_two(p: u8) -> u32 {
    return (1 << p) as u32
}

#[inline]
pub fn get_bucket(precision: u8, hash: u64) -> usize {
    let mask: u64 = match precision {
        0 => return 0, // No bits selected, so hashes map to bucket 0.
        1..=63 => (1u64 << precision) - 1, // Creates a mask with 'precision' number of 1s
        _ => panic!("Invalid percision used"),
    };
    (mask & hash) as usize
}

#[inline]
pub fn longest_run(percision: u8, hash: u64) -> u8 {
    (hash >> percision).trailing_zeros() as u8 + 1
}

#[inline]
pub fn harmonic_mean(registers: &[u8]) -> f64 {
    let sum: f64 = registers.iter().map(|&val| 2.0f64.powi(-(val as i32))).sum();
    1.0 / sum
}

// Bias correction for the given number of registers
#[inline]
pub fn get_alpha_m_bias(m: f64) -> f64 {
    match m {
        4.0 => 0.673, // for m = 16
        5.0 => 0.697, // for m = 32
        6.0 => 0.709, // for m = 64
        _ => 0.7213 / (1.0 + 1.079 / m),
    }
}

