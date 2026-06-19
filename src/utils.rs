
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
    1.04 / (2f64.powi(p as i32)).sqrt()
}


// Convert a relative error value into the corresponding precision
#[inline]
pub fn p_from_rel_error(target_rse: f64) -> u32 {
    assert!(target_rse > 0.0, "target_rse must be > 0");
    let m_target = (1.04 / target_rse).powi(2);
    let p = (m_target.log2().ceil()) as u32;
    p.clamp(4, 25);
    p
}






// Adapted from production FDLIBM implementations using SWAR bit reduction.
// Fully optimized, branchless, ILP-parallelized natural logarithm for no_std.
// Will be used later when implementing no_std
// pub fn no_std_ln(x: f64) -> f64 {
//     let u = x.to_bits();
//     let hx = (u >> 32) as u32;
//     let mut k: i32;

//     // Handle subnormals, zeros, negatives, and special float cases
//     if hx < 0x00100000 {
//         if (u & 0x7FFFFFFFFFFFFFFF) == 0 { return f64::NEG_INFINITY; }
//         if (u & 0x8000000000000000) != 0 { return f64::NAN; }
        
//         // Scale out of subnormal range safely
//         let x_scaled = x * 1.8014398509481984e16; // x * 2^54
//         let u_scaled = x_scaled.to_bits();
//         k = (((u_scaled >> 52) & 0x7FF) as i32) - 1023 - 54;
//     } else if hx >= 0x7FF00000 {
//         return x;
//     } else {
//         k = (hx as i32 >> 20) - 1023;
//     }

//     // Window Reduction via completely branchless bitwise mapping
//     let mantissa_mask = u & 0x000FFFFFFFFFFFFF;
    
//     let shift_mask = (((hx & 0x000FFFFF) | 0x00100000) < 0x000EA09E) as u64;
//     k -= shift_mask as i32;
    
//     // Optimized branchless blend
//     let exp_bias = 0x3FE0000000000000 | (shift_mask << 52);
//     let rem_bits = mantissa_mask | exp_bias;

//     let f = f64::from_bits(rem_bits);
    
//     // Analytical reduction setup
//     let f_minus_1 = f - 1.0;
//     let s = f_minus_1 / (f + 1.0);
//     let z = s * s;
//     let w = z * z;

//     // Polynomial coefficients
//     const P1: f64 = 4.28571428578550184252e-01;
//     const P2: f64 = 2.72728123801472225442e-01;
//     const P3: f64 = 2.06975017800338417784e-01;
    
//     const Q1: f64 = 5.99999999999994648725e-01;
//     const Q2: f64 = 3.33333329818377432918e-01;
//     const Q3: f64 = 2.30660745775561754067e-01;

//     // --- ILP Optimization: Parallel Polynomial Evaluation ---
//     // Expanded out of .mul_add while retaining distinct execution trees
//     let t1_inner = (w * P3) + P2;
//     let t2_inner = (w * Q3) + Q2;

//     let t1 = w * ((w * t1_inner) + P1);
//     let t2 = z * ((w * t2_inner) + Q1);
    
//     let r = t1 + t2;

//     let dk = k as f64;
    
//     // --- Algebraical Elimination ---
//     let hff = 0.5 * f_minus_1 * f_minus_1;
    
//     let inner_poly = (s * r) - hff;
//     let total_poly = f_minus_1 + ((s + s) * inner_poly) + (s * f_minus_1);
    
//     const LN2_HI: f64 = 6.93147180369123816490e-01;
//     const LN2_LO: f64 = 1.90821492927058770002e-10;

//     let log_ln2 = (dk * LN2_LO) + total_poly;
//     (dk * LN2_HI) + log_ln2
// }
