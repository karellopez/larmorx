// SPDX-License-Identifier: Apache-2.0
//! The diagnostics set from the environment at a program's start (spec §4) and the other
//! diagnostics (§5): the diagnostic print, the heartbeat and percent-done lines, the finite
//! check and the break point.

use larmorx_core::cfmt;
use larmorx_core::defects::FS_02;

use crate::clib;
use crate::process::{DIAG_HEARTBEAT, DIAG_VERBOSE, Process, codes};

/// The heartbeat's and the percent-done line's memories of the last fraction printed (§5).
#[derive(Clone, Copy, Debug)]
pub(crate) struct Heartbeat {
    beat: f32,
    pct: f32,
}

impl Default for Heartbeat {
    fn default() -> Self {
        Heartbeat {
            beat: -10.0,
            pct: -10.0,
        }
    }
}

/// FS-02: a double converted to a 32-bit int. `fix`: truncated toward zero, saturated, NaN 0;
/// `reproduce`: x86-64's conversion (beyond the range or NaN: -2147483648). The second value
/// tells whether the two differ.
pub(crate) fn double_to_int(v: f64, reproduce: bool) -> (i32, bool) {
    let in_range = v.is_finite() && v > -2147483649.0 && v < 2147483648.0;
    if in_range {
        return (v as i32, false);
    }
    if reproduce {
        (i32::MIN, true)
    } else {
        // `as` saturates and gives NaN 0
        let s = v as i32;
        (s, s != i32::MIN)
    }
}

impl Process {
    /// FreeSurfer's start-up diagnostics (`DiagInit`, §4): the variables read into the state,
    /// the profiling and verbose lines on standard output; the diagnostic bits returned.
    pub fn diag_init(&mut self) -> u64 {
        let int_var = |p: &Process, name: &str| p.env.get(name).map(clib::atoi);
        if let Some(v) = int_var(self, "DIAG_NO") {
            self.gdiag_no = v;
        }
        if let Some(v) = int_var(self, "IMAGE_SIZE") {
            self.image_size = v;
        }
        for (i, name) in ["DIAGX", "DIAGY", "DIAGZ"].into_iter().enumerate() {
            if let Some(v) = int_var(self, name) {
                self.gvoxel[i] = v;
            }
        }
        for (i, name) in ["DIAGVX", "DIAGVY", "DIAGVZ"].into_iter().enumerate() {
            if let Some(v) = int_var(self, name) {
                self.gvertex[i] = v;
            }
        }
        if let Some(v) = self.env.get("PROFILE").map(clib::atof) {
            let (level, differs) = double_to_int(v, self.mode.reproduce());
            if differs {
                let what = if self.mode.reproduce() {
                    "the profiling level -2147483648 for a PROFILE beyond the int range or NaN"
                } else {
                    "PROFILE's level saturated to the int range (NaN 0)"
                };
                self.note(&FS_02, what);
            }
            self.gprofile = level;
            self.put_out(format!("turning profiling diagnostics on ({level})...\n").as_bytes());
        }
        let bits = self.env.get("diag").or_else(|| self.env.get("DIAG"));
        if let Some(b) = bits
            && let (1, Some(v)) = clib::scan_0x_lx(b)
        {
            self.gdiag |= v;
        }
        if self.env.is_set("DIAG_VERBOSE") {
            self.gdiag |= DIAG_VERBOSE;
        }
        if self.gdiag & DIAG_VERBOSE != 0 {
            self.put_out(b"verbose diagnostics enabled...\n");
        }
        self.gdiag
    }

    /// The diagnostic print (`DiagPrintf(bits, ...)`, the message already formatted): nothing
    /// and -1 when `bits` is not 0 and shares none with the diagnostic bits; else the message on
    /// standard output and its length.
    pub fn diag_printf(&mut self, bits: u64, message: &[u8]) -> i32 {
        if bits != 0 && bits & self.gdiag == 0 {
            return -1;
        }
        self.put_out(message);
        message.len() as i32
    }

    /// The heartbeat (`DiagHeartbeat(fraction)`).
    pub fn diag_heartbeat(&mut self, fraction: f32) {
        if fraction < self.heartbeat.beat {
            self.heartbeat.beat = -20.0;
        }
        if f64::from(fraction - self.heartbeat.beat) > 0.10 {
            self.heartbeat.beat = fraction;
            self.finished_line(fraction);
        }
    }

    /// The percent-done line (`DiagShowPctDone(fraction, n)`): the step `1/n` in float.
    pub fn diag_show_pct_done(&mut self, fraction: f32, n: i32) {
        if fraction < self.heartbeat.pct {
            self.heartbeat.pct = -20.0;
        }
        let step = 1.0f32 / n as f32;
        if fraction - self.heartbeat.pct > step {
            self.heartbeat.pct = fraction;
            self.finished_line(fraction);
        }
    }

    fn finished_line(&mut self, fraction: f32) {
        if self.gdiag & DIAG_HEARTBEAT != 0 {
            let pct = 100.0f32 * fraction;
            let s = cfmt::pad(&cfmt::f(f64::from(pct), 1), 2);
            self.put_err(format!("{s}% finished\n").as_bytes());
        }
    }

    /// The finite check (`check_finite(place, value)`): 1 when finite; else the C-style error
    /// `<place> not finite!` (code -4) and 0.
    pub fn check_finite(&mut self, place: &[u8], value: f64) -> i32 {
        if value.is_finite() {
            return 1;
        }
        let mut m = place.to_vec();
        m.extend_from_slice(b" not finite!\n");
        self.error_printf(codes::BAD_PARM, &m);
        0
    }

    /// The break point (`DiagBreak`): nothing.
    pub fn diag_break(&mut self) {}
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conversions() {
        assert_eq!(double_to_int(7.9, false), (7, false));
        assert_eq!(double_to_int(-2.5, true), (-2, false));
        assert_eq!(double_to_int(2147483647.9, true), (2147483647, false));
        assert_eq!(double_to_int(1e10, true), (i32::MIN, true));
        assert_eq!(double_to_int(1e10, false), (i32::MAX, true));
        assert_eq!(double_to_int(f64::NAN, false), (0, true));
        assert_eq!(double_to_int(-1e10, false), (i32::MIN, false));
    }
}
