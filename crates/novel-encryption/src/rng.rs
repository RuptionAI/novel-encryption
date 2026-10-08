//! Operating-system randomness and unbiased sampling.

use crate::{Error, Result};

/// Fill `buf` from the OS CSPRNG (`getrandom`; `crypto.getRandomValues` on the web).
pub fn fill(buf: &mut [u8]) -> Result<()> {
    getrandom::getrandom(buf).map_err(|e| Error::Rng(e.to_string()))
}

/// Uniform integer in `0..n` by rejection sampling, so no candidate word is
/// favoured by modulo bias. `n` must be non-zero.
pub fn uniform(n: u32) -> Result<u32> {
    assert!(n > 0, "uniform(0)");
    // Largest multiple of n that fits in 2^32; draws at or above it are rejected.
    let zone = (1u64 << 32) - ((1u64 << 32) % n as u64);
    loop {
        let mut b = [0u8; 4];
        fill(&mut b)?;
        let x = u32::from_le_bytes(b) as u64;
        if x < zone {
            return Ok((x % n as u64) as u32);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uniform_stays_in_range_and_covers_it() {
        let mut seen = [0u32; 7];
        for _ in 0..7000 {
            seen[uniform(7).unwrap() as usize] += 1;
        }
        assert!(seen.iter().all(|&c| c > 700), "{seen:?}");
        assert_eq!(uniform(1).unwrap(), 0);
    }
}
