// SPDX-License-Identifier: Apache-2.0
//! The probe's dialects of `larmorx_io::xfm`:
//!
//! - [`ProbeMni`]: volume_io 1.5.1's own behaviour, with the probe's stand-in for MINC volumes
//!   (`up_stubs.c` prints a marker for a grid transform's volume; so does this dialect).
//! - [`ProbeFs`]: FreeSurfer 8.2.0's copy as its behaviour shows it (never its source): only
//!   linear transforms, its own message for another type and for a number that does not convert,
//!   and its own 4 x 4 inverse (called in FreeSurfer's library itself, through the shim, so that
//!   everything else is compared exactly). What it holds is the list of FreeSurfer's changes the
//!   probe measured (`README.md`).

use std::cell::RefCell;

use larmorx_core::cfmt::Streams;
use larmorx_io::xfm::{Dialect, GridVolume, Transform, TransformType};

use crate::{Capture, Lib};

pub struct ProbeMni;

impl Dialect for ProbeMni {
    fn read_grid_volume(&self, filename: &[u8], streams: &mut dyn Streams) -> Result<GridVolume, ()> {
        streams.err(&format!("probe: input_volume({})\n", String::from_utf8_lossy(filename)));
        Err(())
    }
    fn write_grid_volume(&self, filename: &[u8], _v: &GridVolume, streams: &mut dyn Streams) -> Result<(), ()> {
        streams.err(&format!("probe: output_volume({})\n", String::from_utf8_lossy(filename)));
        Ok(())
    }
}

pub struct ProbeFs<'a> {
    pub lib: &'a Lib,
    pub cap: RefCell<&'a mut Capture>,
}

impl Dialect for ProbeFs<'_> {
    /// FreeSurfer prints its messages directly: a NUL byte is printed, not an end.
    fn print_error(&self, text: &str, streams: &mut dyn Streams) {
        streams.err(text);
    }
    /// FreeSurfer prints the mismatch whatever its caller asks, with the token read.
    fn keyword_mismatch(&self, keyword: &[u8], got: Option<&[u8]>, _print: bool, streams: &mut dyn Streams) {
        streams.err(&format!(
            "Expected \"{} =\" got str:{}\n",
            String::from_utf8_lossy(keyword),
            got.map_or("(null)".into(), String::from_utf8_lossy)
        ));
    }
    /// FreeSurfer's token is one C string: a NUL byte ends it.
    fn nul_ends_token(&self) -> bool {
        true
    }
    fn invert(&self, t: &Transform, streams: &mut dyn Streams) -> Option<Transform> {
        let m: Vec<f64> = t.iter().flatten().copied().collect();
        let mut inv = [0.0f64; 16];
        let f = self.lib.compute_transform_inverse.expect("compute_transform_inverse");
        // SAFETY: two 16-double arrays, as the shim reads and writes them.
        let (ok, out, err) = self.cap.borrow_mut().run(|| unsafe { f(m.as_ptr(), inv.as_mut_ptr()) });
        if !out.is_empty() {
            streams.out(&out);
        }
        if !err.is_empty() {
            streams.err(&err);
        }
        if ok == 0 {
            return None;
        }
        let mut r = [[0.0; 4]; 4];
        for i in 0..4 {
            r[i].copy_from_slice(&inv[4 * i..4 * i + 4]);
        }
        Some(r)
    }
    fn transform_type(&self, name: &[u8], streams: &mut dyn Streams) -> Option<TransformType> {
        if name == b"Linear" {
            return Some(TransformType::Linear);
        }
        streams.err(&format!(
            "input_transform(): invalid transform type {}.\n",
            String::from_utf8_lossy(name)
        ));
        None
    }
    fn bad_real(&self, token: &[u8], streams: &mut dyn Streams) {
        streams.err(&format!("{} is not a valid double\n", String::from_utf8_lossy(token)));
    }
}
