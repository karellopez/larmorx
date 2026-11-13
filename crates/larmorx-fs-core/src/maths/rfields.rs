// SPDX-License-Identifier: Apache-2.0
//! Random-field specifications (spec §19): the name's code, the cleared specification with its
//! generator (RANLUX389, `larmorx_core::ranlux`) and its seed (the time of day's for 0: FS-29).
//! The draws come with the volumes.

use larmorx_core::ranlux::Ranlux;

use crate::process::Process;

/// The distributions' codes (`RFname2Code`).
pub mod code {
    pub const UNIFORM: i32 = 1;
    pub const GAUSSIAN: i32 = 2;
    pub const Z: i32 = 3;
    pub const T: i32 = 4;
    pub const F: i32 = 5;
    pub const CHI2: i32 = 6;
}

/// A random-field specification.
#[derive(Clone, Debug)]
pub struct RfSpec {
    pub name: Option<Vec<u8>>,
    pub code: i32,
    pub nparams: i32,
    pub params: [f64; 20],
    pub mean: f64,
    pub stddev: f64,
    pub generator: Ranlux,
    pub seed: u64,
}

/// `RFname2Code(rfs)`: the code of the specification's name (stored and returned), -1 for any
/// other name.
pub fn rf_name_to_code(rfs: &mut RfSpec) -> i32 {
    let c = match rfs.name.as_deref() {
        Some(b"uniform") => code::UNIFORM,
        Some(b"gaussian") => code::GAUSSIAN,
        Some(b"z") => code::Z,
        Some(b"t") => code::T,
        Some(b"F") => code::F,
        Some(b"chi2") => code::CHI2,
        _ => -1,
    };
    rfs.code = c;
    c
}

impl Process {
    /// `RFspecInit(seed, NULL)`: a cleared specification with a new RANLUX389 generator, then
    /// [`Process::rf_spec_set_seed`].
    pub fn rf_spec_init(&mut self, seed: u64) -> RfSpec {
        let mut r = RfSpec {
            name: None,
            code: 0,
            nparams: 0,
            params: [0.0; 20],
            mean: 0.0,
            stddev: 0.0,
            generator: Ranlux::new(larmorx_core::ranlux::DEFAULT_SEED, self.mode()),
            seed: 0,
        };
        self.rf_spec_set_seed(&mut r, seed);
        r
    }

    /// `RFspecSetSeed(rfs, seed)`: 0 takes the time-of-day seed (FS-29); the generator set to
    /// the seed; returns 0.
    pub fn rf_spec_set_seed(&mut self, rfs: &mut RfSpec, seed: u64) -> i32 {
        let s = if seed == 0 { self.pdf_tod_seed() } else { seed };
        rfs.seed = s;
        rfs.generator.reseed(s);
        0
    }
}
