// SPDX-License-Identifier: Apache-2.0
//! FreeSurfer 8.2.0's two changes to gifticlib, as a dialect of the port (read from FreeSurfer's
//! behaviour; this probe models FreeSurfer's side and is closed to its clean-room implementers):
//! the image's version is valid when `sscanf("%f")` reads the same `float` from it as from "1.0"
//! (an absent version reads 0 on both sides), and a coordinate system is accepted under a
//! `NIFTI_INTENT_TIME_SERIES` array too.

use larmorx_core::cfmt::{self, Streams};
use larmorx_core::cscan::sscanf_f32;
use larmorx_io::gifti::{
    Dialect, GIFTI_XML_VERSION, GiftiImage, NIFTI_INTENT_POINTSET, NIFTI_INTENT_TIME_SERIES,
    intent_to_string,
};

pub struct ProbeFs;

impl Dialect for ProbeFs {
    fn check_version(&self, gim: &GiftiImage, whine: bool, out: &mut dyn Streams) -> i32 {
        let (mut v, mut x) = (0.0f32, 0.0f32);
        if let Some(s) = &gim.version {
            if let Some(f) = sscanf_f32(s) {
                v = f;
            }
            if let Some(f) = sscanf_f32(GIFTI_XML_VERSION.as_bytes()) {
                x = f;
            }
        }
        if v != x {
            if whine {
                out.err(&format!(
                    "** invalid: gim version = {}, expected {}\n",
                    cfmt::f(v as f64, 6),
                    cfmt::f(x as f64, 6)
                ));
            }
            return 1;
        }
        0
    }

    fn coordsys_intent_message(&self, da_index: i32, intent: i32) -> Option<String> {
        (intent != NIFTI_INTENT_POINTSET && intent != NIFTI_INTENT_TIME_SERIES).then(|| {
            format!(
                "** DA[{da_index}] has coordsys with intent {} (should be {} or {})\n",
                intent_to_string(intent),
                intent_to_string(NIFTI_INTENT_POINTSET),
                intent_to_string(NIFTI_INTENT_TIME_SERIES)
            )
        })
    }
}
