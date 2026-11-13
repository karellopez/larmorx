// SPDX-License-Identifier: Apache-2.0
//! Random draws (spec §10): the Gaussian draw on the C library's `drand48` (FS-30) and the
//! time-of-day seed (FS-29).

use larmorx_core::defects::{FS_29, FS_30};

use crate::process::Process;
use crate::sse;

impl Process {
    /// `srand48(seed)` of the process's `drand48` state.
    pub fn srand48(&mut self, seed: i64) {
        self.maths.drand48.srand48(seed);
    }

    /// `PDFgaussian()`: Marsaglia's polar method on `drand48`; FS-30 (both draws exactly 0.5).
    pub fn pdf_gaussian(&mut self) -> f64 {
        loop {
            let u1 = self.maths.drand48.drand48();
            let u2 = self.maths.drand48.drand48();
            let v1 = sse::sub64(sse::mul64(2.0, u1), 1.0);
            let v2 = sse::sub64(sse::mul64(2.0, u2), 1.0);
            let r2 = sse::add64(sse::mul64(v1, v1), sse::mul64(v2, v2));
            if r2 > 1.0 {
                continue;
            }
            if r2 == 0.0 {
                if self.mode().fix() {
                    self.note(
                        &FS_30,
                        "both uniform draws exactly 0.5: drawn again instead of a NaN",
                    );
                    continue;
                }
                self.note(&FS_30, "both uniform draws exactly 0.5 gave NaN (0 / 0)");
            }
            let l = self.libm(
                "log",
                r2,
                larmorx_core::math::log,
                larmorx_core::math::log_glibc,
            );
            let f = sse::sqrt64(sse::div64(sse::mul64(-2.0, l), r2));
            return sse::mul64(v1, f);
        }
    }

    /// `PDFtodSeed()`: FS-29, the fixed seed 1 by default, the time of day's seconds plus
    /// microseconds with `reproduce`.
    pub fn pdf_tod_seed(&mut self) -> u64 {
        if self.mode().fix() {
            self.note(&FS_29, "the fixed seed 1 instead of the time of day's");
            return 1;
        }
        self.note(&FS_29, "the seed taken from the time of day");
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default();
        let secs = match self.clock {
            Some(s) => s as u64,
            None => now.as_secs(),
        };
        secs.wrapping_add(u64::from(now.subsec_micros()))
    }
}
