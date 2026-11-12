// SPDX-License-Identifier: Apache-2.0
//! nifticlib 2.0.0's file names and znzlib (ported in `larmorx_io`), against nifticlib 2.0.0 and
//! FreeSurfer 8.2.0's copy.

use std::path::Path;

use crate::{Capture, Lib, Tally};

pub fn check_names(_up: &Lib, _fs: &Lib, _work: &Path, _scale: u64, _cap: &mut Capture) -> Vec<Tally> {
    Vec::new()
}

pub fn check_znz(_up: &Lib, _fs: &Lib, _work: &Path, _scale: u64, _cap: &mut Capture) -> Vec<Tally> {
    Vec::new()
}
