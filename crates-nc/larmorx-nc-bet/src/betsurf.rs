// SPDX-License-Identifier: LicenseRef-FSL
// Copyright 2026 Karel Lopez Vilaret (the translation).
//! `betsurf` (fsl-bet2 2111.8, `betsurf.cpp`): the skull and scalp surfaces.

use std::io::Write;

use crate::bet2::Run;

/// `betsurf`'s main (`args[0]` is the program name).
pub fn main(args: &[String], run: &Run, out: &mut dyn Write, err: &mut dyn Write) -> u8 {
    let _ = (args, run, out);
    let _ = writeln!(err, "betsurf: not implemented yet (larmorx)");
    1
}
