use std::hash::{BuildHasher, Hasher};


const C1: u32 = 0xcc9e2d51;
const C2: u32 = 0x1b873593;

pub struct Murmur3Hasher {
    h1: u32,
    tail: [u8; 4],   // Buffer for the last few bytes
    tail_len: usize, // Number of bytes currently in the tail buffer
    len: usize,      // Total length of bytes processed
}

impl Murmur3Hasher {
    fn new(seed: u32) -> Self {
        Murmur3Hasher {
            h1: seed,
            tail: [0; 4],
            tail_len: 0,
            len: 0,
        }
    }
}

impl Hasher for Murmur3Hasher {
    fn write(&mut self, bytes: &[u8]) {
        self.len += bytes.len();

        let mut data_offset = 0;

        // Process any leftover tail bytes from previous writes
        if self.tail_len > 0 {
            let bytes_to_copy = (4 - self.tail_len).min(bytes.len());
            self.tail[self.tail_len..self.tail_len + bytes_to_copy]
                .copy_from_slice(&bytes[..bytes_to_copy]);
            self.tail_len += bytes_to_copy;
            data_offset += bytes_to_copy;

            // If the tail buffer is full, process it
            if self.tail_len == 4 {
                let k1 = u32::from_le_bytes(self.tail);
                let mut k1 = k1.wrapping_mul(C1);
                k1 = k1.rotate_left(15);
                k1 = k1.wrapping_mul(C2);

                self.h1 ^= k1;
                self.h1 = self.h1.rotate_left(13);
                self.h1 = self.h1.wrapping_mul(5).wrapping_add(0xe6546b64);
                self.tail_len = 0;
            }
        }

        // Process 4-byte chunks from the main data
        let mut i = data_offset;
        let ptr = bytes.as_ptr();
        while i + 4 <= bytes.len() {
            // Use read_unaligned for efficient unaligned reads
            let k1 = unsafe { (ptr.add(i) as *const u32).read_unaligned().to_le() };

            let mut k1 = k1.wrapping_mul(C1);
            k1 = k1.rotate_left(15);
            k1 = k1.wrapping_mul(C2);

            self.h1 ^= k1;
            self.h1 = self.h1.rotate_left(13);
            self.h1 = self.h1.wrapping_mul(5).wrapping_add(0xe6546b64);
            i += 4;
        }

        // Store any remaining bytes in the tail buffer
        let remaining_bytes = bytes.len() - i;
        if remaining_bytes > 0 {
            self.tail[..remaining_bytes].copy_from_slice(&bytes[i..]);
            self.tail_len = remaining_bytes;
        }
    }

    fn finish(&self) -> u64 {
        let mut final_h1 = self.h1;

        // Process remaining bytes (tail)
        let mut k1 = 0u32;
        if self.tail_len > 0 {
            for i in 0..self.tail_len {
                k1 ^= (self.tail[i] as u32) << (8 * i);
            }
            k1 = k1.wrapping_mul(C1);
            k1 = k1.rotate_left(15);
            k1 = k1.wrapping_mul(C2);
            final_h1 ^= k1;
        }

        // Finalization mix (avalanche effect)
        final_h1 ^= self.len as u32;
        final_h1 ^= final_h1.wrapping_shr(16);
        final_h1 = final_h1.wrapping_mul(0x85ebca6b);
        final_h1 ^= final_h1.wrapping_shr(13);
        final_h1 = final_h1.wrapping_mul(0xc2b2ae35);
        final_h1 ^= final_h1.wrapping_shr(16);

        final_h1 as u64
    }
}

/// A `BuildHasher` for `Murmur3Hasher`
#[derive(Default, Debug, Eq, PartialEq)]
pub struct Murmur3BuildHasher {
    seed: u32,
}

impl Murmur3BuildHasher {
    pub fn new(seed: u32) -> Self {
        Murmur3BuildHasher { seed }
    }
}

impl BuildHasher for Murmur3BuildHasher {
    type Hasher = Murmur3Hasher;

    fn build_hasher(&self) -> Self::Hasher {
        Murmur3Hasher::new(self.seed)
    }
}

