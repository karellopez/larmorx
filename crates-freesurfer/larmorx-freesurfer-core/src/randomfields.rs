// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! FreeSurfer's random-field specification (`fs.maths`, queue item 3.1-24), translated from
//! FreeSurfer 8.2.0's `include/randomfields.h` (`RFS`, the `RF_*` codes) and
//! `utils/randomfields.cpp`: `RFname2Code` (34-45), `RFspecInit` (73-87) and `RFspecSetSeed`
//! (100-108).
//!
//! The generator an `RFS` holds is GSL's RANLUX389 in FreeSurfer (`sc_rng_alloc`, `sc_rng_set`,
//! `numerics.cpp`'s copy of GSL 1.8): GPL code, which never comes into this crate (queue item 3.1-53
//! puts GSL's replica alone in `larmorx-gpl`; FreeSurfer's programs here use the clean-room
//! generator of item 3.1-05 once it exists). The specification therefore takes the generator as an
//! [`RfGenerator`]: `RFspecInit` allocates one through the caller's factory and `RFspecSetSeed`
//! seeds it. `RFsynth` and `RFdrawVal` draw from GSL's distributions (items 3.1-05 and 3.1-53) and
//! `RFglobalStats` reads `MRI` volumes (`fs.volume`, items 3.1-35 to 3.1-41): they come with those.

use crate::globals::Globals;
use crate::pdf;

/// `RF_UNIFORM` ... `RF_CHI2`.
pub const RF_UNIFORM: i32 = 1;
pub const RF_GAUSSIAN: i32 = 2;
pub const RF_Z: i32 = 3;
pub const RF_T: i32 = 4;
pub const RF_F: i32 = 5;
pub const RF_CHI2: i32 = 6;

/// The generator an [`Rfs`] draws from (`sc_rng *` with its type): set to a seed.
pub trait RfGenerator {
    /// `sc_rng_set(rng, seed)`.
    fn set(&mut self, seed: u64);
}

/// `RANDOM_FIELD_SPEC` (`RFS`), as `RFspecInit`'s `calloc` leaves it: no name, code 0, no
/// parameters, mean and standard deviation 0.
pub struct Rfs {
    /// `name` (`NULL`: `None`).
    pub name: Option<String>,
    pub code: i32,
    pub nparams: i32,
    pub params: [f64; 20],
    pub mean: f64,
    pub stddev: f64,
    pub rng: Box<dyn RfGenerator>,
    /// `seed` (`unsigned long`).
    pub seed: u64,
}

/// `RFname2Code(rfs)`: the code of the name (`uniform`, `gaussian`, `z`, `t`, `F`, `chi2`; any
/// other -1), stored in `rfs.code` and returned. A spec without a name: FreeSurfer's `strcmp` of
/// `NULL` crashes; here it is -1.
pub fn rf_name2code(rfs: &mut Rfs) -> i32 {
    let code = match rfs.name.as_deref() {
        Some("uniform") => RF_UNIFORM,
        Some("gaussian") => RF_GAUSSIAN,
        Some("z") => RF_Z,
        Some("t") => RF_T,
        Some("F") => RF_F,
        Some("chi2") => RF_CHI2,
        _ => -1,
    };
    rfs.code = code;
    code
}

/// `RFspecSetSeed(rfs, seed)`: `seed`, or [`pdf::pdf_tod_seed`]'s (FS-29) for 0, into `rfs.seed`
/// and the generator. Returns 0.
pub fn rf_spec_set_seed(g: &mut Globals, rfs: &mut Rfs, seed: u64) -> i32 {
    rfs.seed = if seed == 0 {
        pdf::pdf_tod_seed(g)
    } else {
        seed
    };
    rfs.rng.set(rfs.seed);
    0
}

/// `RFspecInit(seed, rngtype)`: a cleared spec with a new generator (`make`, FreeSurfer's only
/// type, RANLUX389, whatever `rngtype` says), seeded by [`rf_spec_set_seed`].
pub fn rf_spec_init(
    g: &mut Globals,
    seed: u64,
    make: impl FnOnce() -> Box<dyn RfGenerator>,
) -> Rfs {
    let mut rfs = Rfs {
        name: None,
        code: 0,
        nparams: 0,
        params: [0.0; 20],
        mean: 0.0,
        stddev: 0.0,
        rng: make(),
        seed: 0,
    };
    rf_spec_set_seed(g, &mut rfs, seed);
    rfs
}

#[cfg(test)]
mod tests {
    use super::*;
    use larmorx_core::defects::{FS_29, UpstreamDefects};
    use std::cell::RefCell;
    use std::rc::Rc;

    struct Probe(Rc<RefCell<Vec<u64>>>);
    impl RfGenerator for Probe {
        fn set(&mut self, seed: u64) {
            self.0.borrow_mut().push(seed);
        }
    }

    #[test]
    fn names_and_seeds() {
        let mut g = Globals::new(UpstreamDefects::Fix);
        let seen = Rc::new(RefCell::new(Vec::new()));
        let s2 = seen.clone();
        let mut rfs = rf_spec_init(&mut g, 53, move || Box::new(Probe(s2)));
        assert_eq!(rfs.seed, 53);
        for (name, code) in [
            ("uniform", 1),
            ("gaussian", 2),
            ("z", 3),
            ("t", 4),
            ("F", 5),
            ("chi2", 6),
            ("f", -1),
        ] {
            rfs.name = Some(name.to_owned());
            assert_eq!(rf_name2code(&mut rfs), code);
            assert_eq!(rfs.code, code);
        }
        rf_spec_set_seed(&mut g, &mut rfs, 0);
        assert_eq!(rfs.seed, pdf::FIXED_SEED);
        assert!(g.notices.contains(&FS_29));
        assert_eq!(*seen.borrow(), [53, pdf::FIXED_SEED]);
    }
}
