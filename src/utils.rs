
// max value that can be passed is 31, which is not a big problem because this is way beyound the supported max percision
#[inline]
pub fn pow_two(p: u8) -> u32 {
    1 << p
}

#[inline]
pub fn get_bucket(precision: u8, hash: u64) -> usize {
    if precision == 0 {
        return 0; // No bits selected, so hashes map to bucket 0.
    }
    let mask = if precision <= 63 {
        (1u64 << precision) - 1 // Creates a mask with 'precision' number of 1s
    } else {
        panic!("Invalid precision used");
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
    if m == 4.0 {
        0.673
    } else if m == 5.0 {
        0.697
    } else if m == 6.0 {
        0.709
    } else {
        0.7213 / (1.0 + 1.079 / m)
    }
}



/// Get the relative error corresponding to a specific p value
#[inline]
pub fn rel_error_from_p(p: u32) -> f64 {
    1.04 / (2f64.powi(p as i32)).sqrt()
}


// Convert a relative error value into the corresponding percision
#[inline]
pub fn p_from_rel_error(target_rse: f64) -> u32 {
    assert!(target_rse > 0.0, "target_rse must be > 0");
    let m_target = (1.04 / target_rse).powi(2);
    let p = (m_target.log2().ceil()) as u32;
    p.clamp(4, 25);
    p
}