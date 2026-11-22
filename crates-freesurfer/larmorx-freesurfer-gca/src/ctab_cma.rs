// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `utils/gca.cpp`'s `GCAcolorTableCMA` (19863-19933): a colour table of the labels a GCA atlas
//! holds (track D's relay 49 of FreeSurfer's foundations, for `mri_segstats --ctab-gca`).
//!
//! FS-1117 (a, memory): FreeSurfer marks each label in a list of 1,000 and names it from
//! `$FREESURFER_HOME/FreeSurferColorLUT.txt`: a label of 1,000 or more is written past the list,
//! and a label the colour table does not hold is named through an entry that does not exist (a
//! crash). Both modes keep every label; one the colour table lacks ends the program with an
//! error (status 1; FreeSurfer's crash, 139, in `reproduce`).

use std::io::Write;
use std::path::Path;

use larmorx_core::defects::FS_1117;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::Fatal;
use larmorx_freesurfer_core::random::random_number;
use larmorx_freesurfer_core::run::Environment;
use larmorx_freesurfer_core::utils::nint;
use larmorx_freesurfer_io::colortab::{ctab_alloc, ctab_read_ascii};
use larmorx_freesurfer_mri::ctab::ColorTable;

use crate::gca::Gca;

/// FreeSurfer's list of labels (`labelhitlist[1000]`).
const LABEL_ROOM: usize = 1000;

/// `GCAcolorTableCMA(gca)`: `GCAcolorTableCMA: using ctab <home>/FreeSurferColorLUT.txt` on
/// standard output (that table read: `ERROR: reading <file>`, status 1, when it does not), then a
/// table of `max label + 1` entries (`CTABalloc`) in which each label the nodes hold gets random
/// colours (red, green, blue: `nint(randomNumber(0, 255))`), alpha 255 and the colour table's
/// name, and every other entry is removed.
pub fn gca_color_table_cma(
    g: &mut Globals,
    env: &Environment,
    out: &mut dyn Write,
    err: &mut dyn Write,
    dir: Option<&Path>,
    gca: &Gca,
) -> Result<ColorTable, Fatal> {
    let home = env
        .get("FREESURFER_HOME")
        .map(|v| v.to_string_lossy().into_owned())
        .unwrap_or_else(|| "(null)".to_owned());
    let ctabfile = format!("{home}/FreeSurferColorLUT.txt");
    let _ = writeln!(out, "GCAcolorTableCMA: using ctab {ctabfile}");
    let Some(ct0) = ctab_read_ascii(g, out, err, dir, &ctabfile) else {
        let _ = writeln!(out, "ERROR: reading {ctabfile}");
        return Err(Fatal::Exit(1));
    };
    let mut hit = vec![false; LABEL_ROOM];
    for node in &gca.nodes {
        for &l in node.labels.iter().take(node.nlabels.max(0) as usize) {
            let l = usize::from(l);
            if l >= hit.len() {
                g.notices.add(
                    &FS_1117,
                    format!("GCAcolorTableCMA: label {l} (FreeSurfer's list holds 1,000 and writes past it)"),
                );
                hit.resize(l + 1, false);
            }
            hit[l] = true;
        }
    }
    let nmax = hit.iter().rposition(|&h| h).unwrap_or(0);
    let Some(mut ct) = ctab_alloc(g, env, out, err, nmax as i32 + 1)? else {
        return Err(Fatal::Exit(1));
    };
    for (n, &h) in hit.iter().enumerate().take(nmax + 1) {
        if !h {
            ct.entries[n] = None;
            continue;
        }
        let x = random_number(g, env, out, err, 0.0, 255.0)?;
        let ri = nint(g, x);
        let x = random_number(g, env, out, err, 0.0, 255.0)?;
        let gi = nint(g, x);
        let x = random_number(g, env, out, err, 0.0, 255.0)?;
        let bi = nint(g, x);
        let Some(name) = ct0
            .entries
            .get(n)
            .and_then(|e| e.as_ref())
            .map(|e| e.name.clone())
        else {
            g.notices.add(
                &FS_1117,
                format!("GCAcolorTableCMA: label {n} is not in {ctabfile} (FreeSurfer crashes)"),
            );
            if g.notices.reproduce() {
                return Err(Fatal::Exit(139));
            }
            let _ = writeln!(
                err,
                "GCAcolorTableCMA: label {n} of the atlas is not in {ctabfile} (FreeSurfer crashes here)"
            );
            return Err(Fatal::Exit(1));
        };
        let e = ct.entries[n].as_mut().expect("CTABalloc's entry");
        e.ri = ri;
        e.gi = gi;
        e.bi = bi;
        e.ai = 255;
        e.rf = (f64::from(ri as f32) / 255.0) as f32;
        e.gf = (f64::from(gi as f32) / 255.0) as f32;
        e.bf = (f64::from(bi as f32) / 255.0) as f32;
        e.af = (f64::from(255.0f32) / 255.0) as f32;
        e.name = name;
    }
    Ok(ct)
}
