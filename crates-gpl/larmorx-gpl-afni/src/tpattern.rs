// SPDX-License-Identifier: GPL-3.0-or-later
// Translated to Rust from AFNI 25.2.09 src/thd_timeof.c (TS_parse_tpattern).
// Major portions Copyright (C) 1994-2000 Medical College of Wisconsin (AFNI, Robert W. Cox et al.),
// released under the GNU GPL version 2 or any later version.
// Rust translation Copyright 2026 Karel Lopez Vilaret; modified from the original.
//! Slice acquisition times from a `-tpattern` value: `TS_parse_tpattern` (`thd_timeof.c`).
//!
//! Named patterns step by `TR/nz` in float, adding the step slice after slice in acquisition
//! order. `@file` reads one time per slice from a 1D text (`mri_read_1D`); a value outside
//! `[0, TR]` there is an error.

use crate::cnum::fmt_g;
use crate::oned;

/// Why a pattern was refused. The messages are AFNI's.
#[derive(Debug, thiserror::Error)]
pub enum TpatternError {
    /// `ERROR_exit("Can't read tpattern file %s")`, with the 1D reader's reason.
    #[error("Can't read tpattern file {file}")]
    Read {
        file: String,
        #[source]
        source: oned::OneDError,
    },
    #[error("tpattern file {file} has {nv} values but have {nz} slices")]
    TooShort { file: String, nv: usize, nz: usize },
    /// `ERROR_exit("Illegal value %g in tpattern file %s")`.
    #[error("Illegal value {} in tpattern file {file}", fmt_g(f64::from(*.value)))]
    Illegal { value: f32, file: String },
    /// `ERROR_message("Unknown tpattern = %s")` (not fatal in AFNI's wording; 3dTshift then
    /// exits with status 1).
    #[error("Unknown tpattern = {0}")]
    Unknown(String),
    #[error("no slices")]
    NoSlices,
}

/// `TS_parse_tpattern(nzz, TR, tpattern)`: the time offset of each of `nzz` slices.
pub fn parse_tpattern(nzz: usize, tr: f32, tpattern: &str) -> Result<Vec<f32>, TpatternError> {
    if nzz < 1 {
        return Err(TpatternError::NoSlices);
    }
    let mut tpat = vec![0.0f32; nzz];
    let tr = if tr < 0.0 { 1.0 } else { tr };
    let tframe = tr / nzz as f32;
    let nz = nzz as i64;
    // Gives the slices in `order` the times 0, tframe, 2*tframe, ... (float additions).
    let assign = |tpat: &mut [f32], order: &mut dyn Iterator<Item = i64>, tsl: &mut f32| {
        for ii in order {
            tpat[ii as usize] = *tsl;
            *tsl += tframe;
        }
    };
    // `for( ii=start ; ii >= 0 ; ii-=2 )` and `for( ii=start ; ii < nzz ; ii+=2 )`.
    let down = |start: i64| (0..=start.max(-1)).rev().step_by(2).filter(|&i| i >= 0);
    let up = |start: i64| (start..nz).step_by(2);
    let mut tsl = 0.0f32;
    if tpattern.is_empty()
        || tpattern.eq_ignore_ascii_case("zero")
        || tpattern.eq_ignore_ascii_case("simult")
    {
        // all zeros
    } else if let Some(file) = tpattern.strip_prefix('@') {
        let tim = oned::read_1d(file).map_err(|source| TpatternError::Read {
            file: file.to_owned(),
            source,
        })?;
        if tim.nx < nzz && tim.ny < nzz && tim.nx * tim.ny < nzz {
            let mut nv = tim.nx * tim.ny;
            if nv == 0 {
                nv = tim.nx.max(tim.ny);
            }
            return Err(TpatternError::TooShort {
                file: file.to_owned(),
                nv,
                nz: nzz,
            });
        }
        for (ii, t) in tpat.iter_mut().enumerate() {
            *t = tim.data[ii];
            if *t < 0.0 || *t > tr {
                return Err(TpatternError::Illegal {
                    value: *t,
                    file: file.to_owned(),
                });
            }
        }
    } else if tpattern == "alt+z" || tpattern == "altplus" {
        assign(&mut tpat, &mut up(0), &mut tsl);
        assign(&mut tpat, &mut up(1), &mut tsl);
    } else if tpattern == "alt+z2" {
        assign(&mut tpat, &mut up(1), &mut tsl);
        assign(&mut tpat, &mut up(0), &mut tsl);
    } else if tpattern == "alt-z" || tpattern == "altminus" {
        assign(&mut tpat, &mut down(nz - 1), &mut tsl);
        assign(&mut tpat, &mut down(nz - 2), &mut tsl);
    } else if tpattern == "alt-z2" {
        assign(&mut tpat, &mut down(nz - 2), &mut tsl);
        assign(&mut tpat, &mut down(nz - 1), &mut tsl);
    } else if tpattern == "seq+z" || tpattern == "seqplus" {
        assign(&mut tpat, &mut (0..nz), &mut tsl);
    } else if tpattern == "seq-z" || tpattern == "seqminus" {
        assign(&mut tpat, &mut (0..nz).rev(), &mut tsl);
    } else {
        return Err(TpatternError::Unknown(tpattern.to_owned()));
    }
    Ok(tpat)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn afni_help_table() {
        // AFNI's -help: nz = 5, TR = 1.
        for (name, want) in [
            ("altplus", [0.0, 0.6, 0.2, 0.8, 0.4]),
            ("alt+z2", [0.4, 0.0, 0.6, 0.2, 0.8]),
            ("altminus", [0.4, 0.8, 0.2, 0.6, 0.0]),
            ("alt-z2", [0.8, 0.2, 0.6, 0.0, 0.4]),
            ("seqplus", [0.0, 0.2, 0.4, 0.6, 0.8]),
            ("seqminus", [0.8, 0.6, 0.4, 0.2, 0.0]),
        ] {
            let t = parse_tpattern(5, 1.0, name).unwrap();
            for (a, b) in t.iter().zip(want) {
                assert!((a - b).abs() < 1e-6, "{name}: {t:?}");
            }
        }
        assert_eq!(parse_tpattern(3, 2.0, "SIMULT").unwrap(), [0.0; 3]);
        assert!(matches!(
            parse_tpattern(3, 2.0, "ALT+Z"),
            Err(TpatternError::Unknown(_))
        ));
        // The step is added in float, slice after slice.
        let t = parse_tpattern(33, 2.0, "seq+z").unwrap();
        let mut s = 0.0f32;
        for v in t {
            assert_eq!(v.to_bits(), s.to_bits());
            s += 2.0f32 / 33.0;
        }
    }

    #[test]
    fn files_are_checked() {
        assert_eq!(
            parse_tpattern(3, 2.0, "@1D: 0 1 0.5 1.5").unwrap(),
            [0.0, 1.0, 0.5]
        );
        assert!(matches!(
            parse_tpattern(3, 2.0, "@1D: 0 1"),
            Err(TpatternError::TooShort { nv: 2, .. })
        ));
        let e = parse_tpattern(3, 2.0, "@1D: 0 2.5 1").unwrap_err();
        assert_eq!(
            e.to_string(),
            "Illegal value 2.5 in tpattern file 1D: 0 2.5 1"
        );
        assert!(parse_tpattern(3, 2.0, "@1D: 0 2 1").is_ok());
    }
}
