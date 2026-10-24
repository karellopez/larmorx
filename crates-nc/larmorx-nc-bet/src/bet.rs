// SPDX-License-Identifier: LicenseRef-FSL
// Copyright 2026 Karel Lopez Vilaret (the translation).
//! The `bet` script (fsl-bet2 2111.8, `bet`): its options, messages and variations.

use std::io::Write;

use crate::bet2::Run;

/// `bet`'s main (`args[0]` is the program name).
pub fn main(args: &[String], run: &Run, out: &mut dyn Write, err: &mut dyn Write) -> u8 {
    let _ = (args, run, out);
    let _ = writeln!(err, "bet: not implemented yet (larmorx)");
    1
}
