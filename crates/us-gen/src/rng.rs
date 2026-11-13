//! Small seeded RNG (SplitMix64). No platform dependence, no external crate.

#[derive(Clone, Debug)]
pub struct Rng(u64);

impl Rng {
    /// Build a stream from a master seed and a salt, so every stage has its own stream.
    pub fn new(seed: u64, salt: u64) -> Rng {
        let mut r = Rng(seed ^ salt.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ 0xD1B5_4A32_D192_ED03);
        r.next_u64();
        r.next_u64();
        r
    }

    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Uniform in [0, 1).
    pub fn f64(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 * (1.0 / 9_007_199_254_740_992.0)
    }

    pub fn uniform(&mut self, a: f64, b: f64) -> f64 {
        a + (b - a) * self.f64()
    }

    /// Integer in [0, n).
    pub fn below(&mut self, n: usize) -> usize {
        if n == 0 {
            return 0;
        }
        (self.next_u64() % n as u64) as usize
    }

    pub fn chance(&mut self, p: f64) -> bool {
        self.f64() < p
    }

    /// Index chosen with the given weights.
    pub fn weighted(&mut self, weights: &[f64]) -> usize {
        let total: f64 = weights.iter().sum();
        let mut x = self.f64() * total;
        for (i, w) in weights.iter().enumerate() {
            if x < *w {
                return i;
            }
            x -= *w;
        }
        weights.len().saturating_sub(1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_stream_is_stable() {
        let mut r = Rng::new(1, 2);
        let a = r.next_u64();
        let mut r2 = Rng::new(1, 2);
        assert_eq!(a, r2.next_u64());
        let mut r3 = Rng::new(2, 2);
        assert_ne!(a, r3.next_u64());
    }

    #[test]
    fn unit_interval() {
        let mut r = Rng::new(9, 9);
        for _ in 0..1000 {
            let x = r.f64();
            assert!((0.0..1.0).contains(&x));
        }
    }
}
