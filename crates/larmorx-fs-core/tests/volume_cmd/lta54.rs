// SPDX-License-Identifier: Apache-2.0
//! The probe's commands of §54's other operations on a transform array (`specs/freesurfer-volume.md`
//! §54: `ltainv`, `ltacat`, `ltacat2`, `ltared`, `ltasrc`, `ltaprt`; the commands and encodings:
//! `validation/investigations/freesurfer-volume-probe/README.md`).

use larmorx_fs_core::maths::Returned;
use larmorx_fs_core::volume::ltachain::LtaInto;
use larmorx_fs_core::{Process, Stop};

use super::args::{Line, bad};
use super::bspline::getlta;
use super::{getw, putlta};

/// One of §54's other commands, or `false` when `c` is none.
pub(super) fn call(p: &mut Process, l: &mut Line, c: &str) -> Result<bool, Stop> {
    match c {
        // LTAinvert into k: 0 none (a new array), 1 the second array, 2 the array itself
        "ltainv" => {
            let k = l.geti();
            let mut lta = getlta(p, l)?;
            let mut given = if k == 1 { Some(getlta(p, l)?) } else { None };
            let into = match k {
                0 => LtaInto::New,
                1 => LtaInto::Given(given.as_mut().expect("the given array")),
                2 => LtaInto::Itself,
                _ => bad("ltainv's k must be 0, 1 or 2"),
            };
            let r = p.lta_invert(&mut lta, into)?;
            match r {
                Returned::Nothing => {
                    l.out("null");
                    putlta(l, None);
                    if k == 1 {
                        putlta(l, given.as_ref());
                    }
                    putlta(l, Some(&lta));
                }
                Returned::New(n) => {
                    l.out("new");
                    putlta(l, Some(&n));
                    putlta(l, Some(&lta));
                }
                Returned::Output if k == 1 => {
                    l.out("given");
                    putlta(l, given.as_ref());
                    putlta(l, Some(&lta));
                }
                Returned::Output => {
                    l.out("self");
                    putlta(l, Some(&lta));
                }
            }
        }
        // LTAconcat of the n arrays, LTAconcat2 of two, with the reduction (1) or not (0)
        "ltacat" | "ltacat2" => {
            let n = if c == "ltacat2" { 2 } else { l.geti() };
            let mut v = Vec::new();
            for _ in 0..n.max(0) {
                v.push(getlta(p, l)?);
            }
            let reduce = l.geti() != 0;
            let r = if c == "ltacat2" {
                let (a, b) = v.split_at_mut(1);
                p.lta_concat2(&mut a[0], &mut b[0], reduce)?
            } else {
                p.lta_concat(&mut v, reduce)?
            };
            putlta(l, r.as_ref());
            for a in &v {
                putlta(l, Some(a));
            }
        }
        "ltared" => {
            let lta = getlta(p, l)?;
            let r = p.lta_reduce(&lta)?;
            putlta(l, r.as_ref());
            putlta(l, Some(&lta));
        }
        "ltasrc" => {
            let lta = getlta(p, l)?;
            let m = getw(p, l)?;
            let s = p.lta_mri_is_source(&lta, &m)?;
            let t = p.lta_mri_is_target(&lta, &m)?;
            l.puti(i64::from(s));
            l.puti(i64::from(t));
        }
        "ltaprt" => {
            let lta = getlta(p, l)?;
            let r = p.lta_print(&lta)?;
            l.puti(i64::from(r));
        }
        _ => return Ok(false),
    }
    Ok(true)
}
