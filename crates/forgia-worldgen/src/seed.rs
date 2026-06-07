//! Hierarchical seeds — deterministic world generation (story-578 P4).
//!
//! `world_seed → region → chunk → parcel → building`. Each level hashes the parent seed with a
//! coordinate or index (one [`derive`] step), so:
//! - the same `world_seed` always reproduces the same world,
//! - any chunk can be regenerated in isolation (streaming) without generating its neighbours,
//! - local variation is cheap (one hash per level).
//!
//! Pure, no ECS, no dependency. The RNG is splitmix64 (fast, good distribution, deterministic).

/// Deterministic splitmix64 RNG. Same seed → same stream.
pub struct SeededRng(u64);

impl SeededRng {
    pub fn new(seed: u64) -> Self {
        Self(seed)
    }

    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Uniform in `[0, 1)`.
    pub fn next_f32(&mut self) -> f32 {
        (self.next_u64() >> 40) as f32 / (1u64 << 24) as f32
    }

    /// Uniform signed in `[-1, 1)`.
    pub fn next_signed(&mut self) -> f32 {
        self.next_f32() * 2.0 - 1.0
    }

    /// Uniform index in `[0, n)` (0 if `n == 0`).
    pub fn below(&mut self, n: usize) -> usize {
        if n == 0 {
            0
        } else {
            (self.next_u64() % n as u64) as usize
        }
    }
}

/// One hierarchy step: mix a parent seed with a child index/coordinate.
pub fn derive(parent: u64, index: u64) -> u64 {
    let mut z = parent ^ index.wrapping_mul(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// Seed for the chunk at integer coordinate `(cx, cy)` under `world_seed`.
pub fn chunk_seed(world_seed: u64, cx: i32, cy: i32) -> u64 {
    let packed = (u64::from(cx as u32) << 32) | u64::from(cy as u32);
    derive(world_seed, packed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rng_is_deterministic() {
        let mut a = SeededRng::new(42);
        let mut b = SeededRng::new(42);
        for _ in 0..100 {
            assert_eq!(a.next_u64(), b.next_u64());
        }
        let mut c = SeededRng::new(43);
        assert_ne!(SeededRng::new(42).next_u64(), c.next_u64());
    }

    #[test]
    fn rng_ranges() {
        let mut r = SeededRng::new(7);
        for _ in 0..1000 {
            let f = r.next_f32();
            assert!((0.0..1.0).contains(&f));
            let s = r.next_signed();
            assert!((-1.0..1.0).contains(&s));
            assert!(r.below(5) < 5);
        }
        assert_eq!(r.below(0), 0);
    }

    #[test]
    fn chunk_seeds_are_stable_and_distinct() {
        assert_eq!(chunk_seed(100, 3, -7), chunk_seed(100, 3, -7), "stable");
        assert_ne!(chunk_seed(100, 3, -7), chunk_seed(100, -7, 3), "coord order matters");
        assert_ne!(chunk_seed(100, 3, -7), chunk_seed(101, 3, -7), "world seed matters");
        // No collisions across a small grid.
        let mut seen = std::collections::HashSet::new();
        for x in -5..5 {
            for y in -5..5 {
                assert!(seen.insert(chunk_seed(1, x, y)), "no collision at ({x},{y})");
            }
        }
    }
}
