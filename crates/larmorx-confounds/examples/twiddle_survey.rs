// SPDX-License-Identifier: Apache-2.0
//! Whether glibc 2.35's `sincos` (`larmorx_core::math::sincos_glibc`, what SciPy's FFT twiddles
//! call) and the correctly rounded `sin`/`cos` give the same single-precision twiddles of every
//! power-of-two length up to `2^<max_log2>` (default 30): the double-precision roots compared,
//! and every float entry of a table whose roots differ.
//!
//!     cargo run --release -p larmorx-confounds --example twiddle_survey -- [max_log2]

fn main() {
    let k: u32 = std::env::args()
        .nth(1)
        .map_or(30, |a| a.parse().expect("max_log2"));
    let (lengths, roots, roots_diff, entries, entries_diff) =
        larmorx_confounds::scipy_fft::twiddle_survey(k);
    println!(
        "lengths 2^0..=2^{k} ({lengths}): double roots compared {roots}, differing {roots_diff}; \
         float entries {entries}, differing {entries_diff}"
    );
    if entries_diff != 0 {
        std::process::exit(1);
    }
}
