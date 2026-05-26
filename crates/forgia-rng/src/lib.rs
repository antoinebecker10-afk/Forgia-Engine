//! # forgia-rng
//!
//! Seeded deterministic RNG for procgen — **pure data, zero Bevy dep**.
//! Implementation : **xoshiro256++** (Blackman & Vigna 2018).
//!
//! Why not `rand` or `bevy_rand` :
//! - Procgen generators run in pure crates (e.g. `forgia-village-generator`)
//!   that must compile fast, be testable in isolation, and reproducible.
//! - We need bit-identical output across runs and platforms — that's the
//!   xoshiro256++ design goal (vs. ChaCha20 in `rand` which is heavier).
//! - Pure trait-free API : no `RngCore` trait, no allocations, no panics.
//!
//! ## Reproducibility guarantee
//!
//! `Rng::new(seed)` followed by N identical `next_*` calls always returns
//! bit-identical sequences. Two `Rng` instances with the same seed produce
//! the same stream forever. Validated by `tests::reproducibility_seed_42`.
//!
//! ## Source
//!
//! - xoshiro256++ reference: <https://prng.di.unimi.it/>
//! - splitmix64 (for seeding): <https://prng.di.unimi.it/splitmix64.c>

/// xoshiro256++ state (4 × u64).
#[derive(Clone, Debug)]
pub struct Rng {
    state: [u64; 4],
}

impl Rng {
    /// Initialize from a 64-bit seed. Uses splitmix64 to populate the 4-word
    /// state — xoshiro256++ author's recommended seeding method.
    ///
    /// `seed = 0` is allowed but the state must not be all zeros (it's a
    /// fixed point of the xoshiro recurrence). splitmix64 with input 0
    /// returns non-zero, so this is safe.
    pub fn new(seed: u64) -> Self {
        let mut sm = SplitMix64 { state: seed };
        Self {
            state: [sm.next(), sm.next(), sm.next(), sm.next()],
        }
    }

    /// Raw u64 — the building block. Other methods derive from this.
    ///
    /// Algorithm : xoshiro256++ verbatim from Blackman & Vigna 2018.
    pub fn next_u64(&mut self) -> u64 {
        let result = self.state[0]
            .wrapping_add(self.state[3])
            .rotate_left(23)
            .wrapping_add(self.state[0]);

        let t = self.state[1] << 17;

        self.state[2] ^= self.state[0];
        self.state[3] ^= self.state[1];
        self.state[1] ^= self.state[2];
        self.state[0] ^= self.state[3];

        self.state[2] ^= t;
        self.state[3] = self.state[3].rotate_left(45);

        result
    }

    /// Uniform `f32` in `[0, 1)` — uses top 24 bits of u64 (mantissa size).
    pub fn next_f32(&mut self) -> f32 {
        // Take top 24 bits, divide by 2^24 — gives [0, 1).
        let x = (self.next_u64() >> 40) as u32;
        x as f32 / (1u32 << 24) as f32
    }

    /// Uniform `f64` in `[0, 1)` — uses top 53 bits (f64 mantissa).
    pub fn next_f64(&mut self) -> f64 {
        let x = self.next_u64() >> 11;
        x as f64 / (1u64 << 53) as f64
    }

    /// Uniform `f32` in `[lo, hi)`. Returns `lo` if `hi <= lo`.
    pub fn range_f32(&mut self, lo: f32, hi: f32) -> f32 {
        if hi <= lo {
            return lo;
        }
        lo + self.next_f32() * (hi - lo)
    }

    /// Uniform `u32` in `[lo, hi)`. Returns `lo` if `hi <= lo`.
    pub fn range_u32(&mut self, lo: u32, hi: u32) -> u32 {
        if hi <= lo {
            return lo;
        }
        let range = u64::from(hi - lo);
        lo + (self.next_u64() % range) as u32
    }

    /// Uniform `usize` in `[lo, hi)`. Returns `lo` if `hi <= lo`.
    pub fn range_usize(&mut self, lo: usize, hi: usize) -> usize {
        if hi <= lo {
            return lo;
        }
        let range = (hi - lo) as u64;
        lo + (self.next_u64() % range) as usize
    }

    /// Bernoulli trial — returns `true` with probability `prob` in `[0, 1]`.
    pub fn bool(&mut self, prob: f32) -> bool {
        self.next_f32() < prob.clamp(0.0, 1.0)
    }

    /// Fisher-Yates shuffle in-place. Deterministic for a given Rng state.
    pub fn shuffle<T>(&mut self, slice: &mut [T]) {
        let n = slice.len();
        for i in (1..n).rev() {
            let j = self.range_usize(0, i + 1);
            slice.swap(i, j);
        }
    }

    /// Uniform sample from a slice. Returns `None` for empty input.
    pub fn pick<'a, T>(&mut self, items: &'a [T]) -> Option<&'a T> {
        if items.is_empty() {
            return None;
        }
        let idx = self.range_usize(0, items.len());
        Some(&items[idx])
    }

    /// Weighted sample — picks an index proportional to its weight. Weights
    /// must be non-negative and not all zero. Returns `None` if invalid.
    pub fn weighted_pick(&mut self, weights: &[f32]) -> Option<usize> {
        let total: f32 = weights.iter().filter(|w| **w > 0.0).sum();
        if total <= 0.0 || !total.is_finite() {
            return None;
        }
        let target = self.next_f32() * total;
        let mut acc = 0.0_f32;
        for (i, w) in weights.iter().enumerate() {
            if *w <= 0.0 {
                continue;
            }
            acc += *w;
            if target < acc {
                return Some(i);
            }
        }
        // Floating-point edge case : return last positive-weight index.
        weights.iter().rposition(|w| *w > 0.0)
    }

    /// Sample a uniform unit-disc point in 2D — for Poisson disc fallback,
    /// rotation seeds, etc. Returns `(x, y)` in unit disc.
    pub fn unit_disc(&mut self) -> (f32, f32) {
        loop {
            let x = self.range_f32(-1.0, 1.0);
            let y = self.range_f32(-1.0, 1.0);
            if x * x + y * y <= 1.0 {
                return (x, y);
            }
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// splitmix64 — used only for seeding xoshiro256++.
// ─────────────────────────────────────────────────────────────────────────────

struct SplitMix64 {
    state: u64,
}

impl SplitMix64 {
    fn next(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E3779B97F4A7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
        z ^ (z >> 31)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reproducibility_seed_42() {
        let mut a = Rng::new(42);
        let mut b = Rng::new(42);
        for _ in 0..1000 {
            assert_eq!(a.next_u64(), b.next_u64());
        }
    }

    #[test]
    fn different_seeds_differ() {
        let mut a = Rng::new(42);
        let mut b = Rng::new(43);
        assert_ne!(a.next_u64(), b.next_u64());
    }

    #[test]
    fn seed_zero_works() {
        let mut r = Rng::new(0);
        let v = r.next_u64();
        assert_ne!(v, 0, "seed 0 must NOT produce all-zero stream");
    }

    #[test]
    fn range_f32_within_bounds() {
        let mut r = Rng::new(42);
        for _ in 0..10_000 {
            let v = r.range_f32(-5.0, 7.5);
            assert!((-5.0..7.5).contains(&v));
        }
    }

    #[test]
    fn range_usize_within_bounds() {
        let mut r = Rng::new(42);
        for _ in 0..10_000 {
            let v = r.range_usize(10, 20);
            assert!((10..20).contains(&v));
        }
    }

    #[test]
    fn shuffle_is_permutation() {
        let mut r = Rng::new(42);
        let mut v: Vec<u32> = (0..50).collect();
        let original_sum: u32 = v.iter().sum();
        r.shuffle(&mut v);
        let new_sum: u32 = v.iter().sum();
        assert_eq!(original_sum, new_sum, "shuffle must preserve elements");
        assert!(v != (0..50).collect::<Vec<_>>(), "shuffle should reorder");
    }

    #[test]
    fn pick_empty_returns_none() {
        let mut r = Rng::new(42);
        let empty: [i32; 0] = [];
        assert!(r.pick(&empty).is_none());
    }

    #[test]
    fn weighted_pick_respects_weights() {
        let mut r = Rng::new(42);
        let mut counts = [0u32; 3];
        for _ in 0..10_000 {
            let i = r.weighted_pick(&[0.0, 9.0, 1.0]).unwrap();
            counts[i] += 1;
        }
        assert_eq!(counts[0], 0, "zero weight must never be picked");
        assert!(
            counts[1] > 8500 && counts[1] < 9500,
            "bucket 1 ~9000, got {}",
            counts[1]
        );
        assert!(
            counts[2] > 500 && counts[2] < 1500,
            "bucket 2 ~1000, got {}",
            counts[2]
        );
    }

    #[test]
    fn unit_disc_inside_disc() {
        let mut r = Rng::new(42);
        for _ in 0..1000 {
            let (x, y) = r.unit_disc();
            assert!(x * x + y * y <= 1.0);
        }
    }

    #[test]
    fn bool_probability_distribution() {
        let mut r = Rng::new(42);
        let mut trues = 0u32;
        for _ in 0..10_000 {
            if r.bool(0.3) {
                trues += 1;
            }
        }
        assert!(
            trues > 2700 && trues < 3300,
            "bool(0.3) gave {} true out of 10000",
            trues
        );
    }
}
