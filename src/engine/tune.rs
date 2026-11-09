//! Search parameters that can be tuned, for example with SPSA.
//!
//! Each parameter is a function returning its value. Normally that's a constant, so tuning costs
//! nothing. With the `tune` feature, each one is also a UCI spin option that can be set at runtime.

#[cfg(feature = "tune")]
use std::sync::atomic::{AtomicI32, Ordering};

/// A search parameter exposed as a UCI option.
#[cfg(feature = "tune")]
pub struct Tunable {
    pub name: &'static str,
    pub default: i32,
    pub min: i32,
    pub max: i32,
    value: &'static AtomicI32,
}

#[cfg(feature = "tune")]
impl Tunable {
    /// Sets the parameter, clamped to its range.
    pub fn set(&self, value: i32) {
        self.value
            .store(value.clamp(self.min, self.max), Ordering::Relaxed);
    }
}

macro_rules! tunables {
    ($(($function:ident, $name:literal, $default:literal, $min:literal, $max:literal),)*) => {
        $(
            #[cfg(not(feature = "tune"))]
            #[inline(always)]
            pub const fn $function() -> i32 {
                $default
            }

            #[cfg(feature = "tune")]
            mod $function {
                pub static VALUE: super::AtomicI32 = super::AtomicI32::new($default);
            }

            #[cfg(feature = "tune")]
            #[inline(always)]
            pub fn $function() -> i32 {
                $function::VALUE.load(Ordering::Relaxed)
            }
        )*

        /// Every tunable search parameter.
        #[cfg(feature = "tune")]
        pub static TUNABLES: &[Tunable] = &[
            $(Tunable {
                name: $name,
                default: $default,
                min: $min,
                max: $max,
                value: &$function::VALUE,
            },)*
        ];
    };
}

tunables! {
    (aspiration_delta, "AspirationDelta", 25, 5, 100),
    (rfp_margin, "RfpMargin", 80, 20, 200),
    (rfp_improving, "RfpImproving", 60, 0, 150),
    (nmp_base, "NmpBase", 5, 2, 8),
    (nmp_depth_divisor, "NmpDepthDivisor", 5, 2, 10),
    (nmp_eval_divisor, "NmpEvalDivisor", 200, 50, 500),
    (lmp_base, "LmpBase", 3, 1, 10),
    (futility_base, "FutilityBase", 200, 50, 400),
    (futility_depth, "FutilityDepth", 100, 25, 250),
    (see_quiet_margin, "SeeQuietMargin", 150, 25, 300),
    (see_noisy_margin, "SeeNoisyMargin", 200, 50, 400),
    (history_bonus_scale, "HistoryBonusScale", 350, 100, 800),
    (history_bonus_max, "HistoryBonusMax", 2000, 500, 4000),
    (qsearch_see_margin, "QsearchSeeMargin", 500, 100, 1000),
}

#[cfg(all(test, feature = "tune"))]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_within_range() {
        for tunable in TUNABLES {
            assert!(
                (tunable.min..=tunable.max).contains(&tunable.default),
                "{}",
                tunable.name
            );
        }
    }

    #[test]
    fn set_clamps_to_range() {
        let tunable = TUNABLES
            .iter()
            .find(|t| t.name == "QsearchSeeMargin")
            .unwrap();

        tunable.set(1_000_000);
        assert_eq!(qsearch_see_margin(), tunable.max);

        tunable.set(tunable.default);
        assert_eq!(qsearch_see_margin(), tunable.default);
    }
}
