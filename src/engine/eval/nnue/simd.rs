//! The net's inner loops, once per instruction set. Every version computes exactly what the scalar
//! one does, wrapping on overflow the way release builds of the scalar loops do, so the choice
//! never changes an eval.
//!
//! The SCReLU dot product multiplies `clamp(v, 0, QA) * w` in 16 bits, then multiply-adds that by
//! `clamp(v, 0, QA)` into 32 bits. That's exact only while `|w| <= SIMD_WEIGHT_LIMIT`, so the
//! SIMD versions take weights already clamped to it, and the caller adds the rest back.

use super::{QA, screlu};

/// The largest output weight `w` for which `QA * w` fits in an i16.
pub(super) const SIMD_WEIGHT_LIMIT: i16 = (i16::MAX as i32 / QA) as i16;

/// The SIMD loops step through 32 values at a time.
pub(super) const CHUNK: usize = 32;

/// Which instruction set the net's loops use.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Simd {
    Scalar,
    #[cfg(target_arch = "x86_64")]
    Avx2,
    #[cfg(target_arch = "aarch64")]
    Neon,
}

impl Simd {
    /// Every version this CPU can run, slowest first.
    pub fn available() -> Vec<Self> {
        #[allow(unused_mut)]
        let mut levels = vec![Self::Scalar];
        #[cfg(target_arch = "x86_64")]
        if std::arch::is_x86_feature_detected!("avx2") {
            levels.push(Self::Avx2);
        }
        #[cfg(target_arch = "aarch64")]
        levels.push(Self::Neon);
        levels
    }

    /// The fastest version this CPU can run.
    pub fn detect() -> Self {
        *Self::available().last().unwrap()
    }
}

pub(super) mod scalar {
    use super::screlu;

    /// Adds (or with `ADD` false, subtracts) a feature's weights to both perspectives.
    #[inline]
    pub fn update<const H: usize, const ADD: bool>(
        white: &mut [i16; H],
        black: &mut [i16; H],
        white_weights: &[i16; H],
        black_weights: &[i16; H],
    ) {
        for i in 0..H {
            if ADD {
                white[i] = white[i].wrapping_add(white_weights[i]);
                black[i] = black[i].wrapping_add(black_weights[i]);
            } else {
                white[i] = white[i].wrapping_sub(white_weights[i]);
                black[i] = black[i].wrapping_sub(black_weights[i]);
            }
        }
    }

    /// `Σ screlu(us) * weights[0] + Σ screlu(them) * weights[1]`, for any weights.
    #[inline]
    pub fn screlu_dot<const H: usize>(
        us: &[i16; H],
        them: &[i16; H],
        weights: &[[i16; H]; 2],
    ) -> i32 {
        let mut output = 0i32;
        for i in 0..H {
            output = output.wrapping_add(screlu(us[i]).wrapping_mul(weights[0][i] as i32));
            output = output.wrapping_add(screlu(them[i]).wrapping_mul(weights[1][i] as i32));
        }
        output
    }
}

#[cfg(target_arch = "x86_64")]
pub(super) mod avx2 {
    use std::arch::x86_64::*;

    use super::{CHUNK, QA};

    const LANES: usize = 16;

    /// Adds (or with `ADD` false, subtracts) a feature's weights to both perspectives.
    #[target_feature(enable = "avx2")]
    pub fn update<const H: usize, const ADD: bool>(
        white: &mut [i16; H],
        black: &mut [i16; H],
        white_weights: &[i16; H],
        black_weights: &[i16; H],
    ) {
        const { assert!(H.is_multiple_of(CHUNK)) };
        for (acc, weights) in [(white, white_weights), (black, black_weights)] {
            for i in (0..H).step_by(LANES) {
                // SAFETY: `i + LANES <= H`, because `H` is a multiple of `LANES`.
                unsafe {
                    let a = _mm256_loadu_si256(acc.as_ptr().add(i).cast());
                    let w = _mm256_loadu_si256(weights.as_ptr().add(i).cast());
                    let r = if ADD {
                        _mm256_add_epi16(a, w)
                    } else {
                        _mm256_sub_epi16(a, w)
                    };
                    _mm256_storeu_si256(acc.as_mut_ptr().add(i).cast(), r);
                }
            }
        }
    }

    /// `Σ screlu(us) * weights[0] + Σ screlu(them) * weights[1]`, for weights within
    /// `±SIMD_WEIGHT_LIMIT`.
    #[target_feature(enable = "avx2")]
    pub fn screlu_dot<const H: usize>(
        us: &[i16; H],
        them: &[i16; H],
        weights: &[[i16; H]; 2],
    ) -> i32 {
        const { assert!(H.is_multiple_of(CHUNK)) };
        let zero = _mm256_setzero_si256();
        let qa = _mm256_set1_epi16(QA as i16);
        let mut sum0 = _mm256_setzero_si256();
        let mut sum1 = _mm256_setzero_si256();
        for (acc, weights) in [(us, &weights[0]), (them, &weights[1])] {
            for i in (0..H).step_by(2 * LANES) {
                // SAFETY: `i + 2 * LANES <= H`, because `H` is a multiple of `CHUNK`.
                let (v0, v1, w0, w1) = unsafe {
                    (
                        _mm256_loadu_si256(acc.as_ptr().add(i).cast()),
                        _mm256_loadu_si256(acc.as_ptr().add(i + LANES).cast()),
                        _mm256_loadu_si256(weights.as_ptr().add(i).cast()),
                        _mm256_loadu_si256(weights.as_ptr().add(i + LANES).cast()),
                    )
                };
                let c0 = _mm256_min_epi16(_mm256_max_epi16(v0, zero), qa);
                let c1 = _mm256_min_epi16(_mm256_max_epi16(v1, zero), qa);
                // c * w fits in 16 bits, and madd sums two of c * (c * w) into each 32-bit lane.
                sum0 = _mm256_add_epi32(sum0, _mm256_madd_epi16(c0, _mm256_mullo_epi16(c0, w0)));
                sum1 = _mm256_add_epi32(sum1, _mm256_madd_epi16(c1, _mm256_mullo_epi16(c1, w1)));
            }
        }
        let sum = _mm256_add_epi32(sum0, sum1);
        let sum = _mm_add_epi32(
            _mm256_castsi256_si128(sum),
            _mm256_extracti128_si256::<1>(sum),
        );
        let sum = _mm_add_epi32(sum, _mm_shuffle_epi32::<0b01_00_11_10>(sum));
        let sum = _mm_add_epi32(sum, _mm_shuffle_epi32::<0b10_11_00_01>(sum));
        _mm_cvtsi128_si32(sum)
    }
}

#[cfg(target_arch = "aarch64")]
pub(super) mod neon {
    use std::arch::aarch64::*;

    use super::{CHUNK, QA};

    const LANES: usize = 8;

    /// Adds (or with `ADD` false, subtracts) a feature's weights to both perspectives.
    #[inline]
    pub fn update<const H: usize, const ADD: bool>(
        white: &mut [i16; H],
        black: &mut [i16; H],
        white_weights: &[i16; H],
        black_weights: &[i16; H],
    ) {
        const { assert!(H.is_multiple_of(CHUNK)) };
        #[inline(always)]
        fn op<const ADD: bool>(a: int16x8_t, w: int16x8_t) -> int16x8_t {
            // SAFETY: NEON is part of the aarch64 baseline.
            unsafe {
                if ADD {
                    vaddq_s16(a, w)
                } else {
                    vsubq_s16(a, w)
                }
            }
        }
        for i in (0..H).step_by(CHUNK) {
            for (acc, weights) in [(&mut *white, white_weights), (&mut *black, black_weights)] {
                // SAFETY: NEON is part of the aarch64 baseline, and `i + CHUNK <= H`, because `H`
                // is a multiple of `CHUNK`.
                unsafe {
                    let a = vld1q_s16_x4(acc.as_ptr().add(i));
                    let w = vld1q_s16_x4(weights.as_ptr().add(i));
                    let r = int16x8x4_t(
                        op::<ADD>(a.0, w.0),
                        op::<ADD>(a.1, w.1),
                        op::<ADD>(a.2, w.2),
                        op::<ADD>(a.3, w.3),
                    );
                    vst1q_s16_x4(acc.as_mut_ptr().add(i), r);
                }
            }
        }
    }

    /// `Σ screlu(us) * weights[0] + Σ screlu(them) * weights[1]`, for weights within
    /// `±SIMD_WEIGHT_LIMIT`.
    #[inline]
    pub fn screlu_dot<const H: usize>(
        us: &[i16; H],
        them: &[i16; H],
        weights: &[[i16; H]; 2],
    ) -> i32 {
        const { assert!(H.is_multiple_of(CHUNK)) };
        // SAFETY: NEON is part of the aarch64 baseline, and `i + CHUNK <= H`, because `H` is a
        // multiple of `CHUNK`.
        unsafe {
            let zero = vdupq_n_s16(0);
            let qa = vdupq_n_s16(QA as i16);
            // Eight sums, so that the multiply-adds don't wait on each other.
            let mut sums = [vdupq_n_s32(0); 2 * CHUNK / LANES];
            for (acc, weights) in [(us, &weights[0]), (them, &weights[1])] {
                for i in (0..H).step_by(CHUNK) {
                    for j in 0..CHUNK / LANES {
                        let v = vld1q_s16(acc.as_ptr().add(i + j * LANES));
                        let w = vld1q_s16(weights.as_ptr().add(i + j * LANES));
                        let c = vminq_s16(vmaxq_s16(v, zero), qa);
                        // c * w fits in 16 bits.
                        let cw = vmulq_s16(c, w);
                        sums[2 * j] = vmlal_s16(sums[2 * j], vget_low_s16(c), vget_low_s16(cw));
                        sums[2 * j + 1] = vmlal_high_s16(sums[2 * j + 1], c, cw);
                    }
                }
            }
            let mut sum = sums[0];
            for s in &sums[1..] {
                sum = vaddq_s32(sum, *s);
            }
            vaddvq_s32(sum)
        }
    }
}
