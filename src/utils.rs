
// max value that can be passed is 31, which is not a big problem because this is way beyound the supported max precision
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
pub fn longest_run(precision: u8, hash: u64) -> u8 {
    (hash >> precision).trailing_zeros() as u8 + 1
}


// A lookup table for 2.0^(-val) where val is 0..=64
const TWO_POW_NEG: [f64; 65] = {
    let mut table = [0.0; 65];
    let mut i = 0;
    while i < 65 {
        // We can use a bitwise raw manipulation trick to generate 2^(-i) at compile time
        table[i] = f64::from_bits((1023 - i as u64) << 52);
        i += 1;
    }
    table
};

#[inline]
pub fn harmonic_mean(registers: &[u8]) -> f64 {
    let sum: f64 = registers.iter().map(|&val| TWO_POW_NEG[val as usize]).sum();
    1.0 / sum
}

// Bias correction for the given number of registers
#[inline]
pub fn get_alpha_m_bias(m: f64) -> f64 {
    match m as u64 {
        4 => 0.673,
        5 => 0.697,
        6 => 0.709,
        _ => 0.7213 / (1.0 + 1.079 / m),
    }
}



/// Get the relative error corresponding to a specific p value
#[inline]
pub fn rel_error_from_p(p: u32) -> f64 {
    #[cfg(not(feature = "no_std"))]
    {
        1.04 / (2f64.powi(p as i32)).sqrt()
    }
    #[cfg(feature = "no_std")]
    {
        1.04 / libm::sqrt(libm::pow(2f64, p as f64))
    }
}


/// Convert a relative error value into the corresponding precision
#[inline]
pub fn p_from_rel_error(target_rse: f64) -> u32 {
    assert!(target_rse > 0.0, "target_rse must be > 0");

    #[cfg(not(feature = "no_std"))]
    {
        let m_target = (1.04 / target_rse).powi(2);
        let p = (m_target.log2().ceil()) as u32;
        p.clamp(4, 25)
    }
    #[cfg(feature = "no_std")]
    {
        let m_target = libm::pow(1.04 / target_rse, 2.0);
        let p = libm::ceil(libm::log2(m_target)) as u32;
        p.clamp(4, 25)
    }
}



