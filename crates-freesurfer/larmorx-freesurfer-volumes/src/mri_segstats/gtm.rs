// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `utils/gtm.cpp`'s replacement lists, which only `mri_segstats` of the programs ported so far
//! reaches (`GTMdefaultSegReplacmentList` 306-391, `GTMloadReplacmentList` 2516-2551, with fs.core's
//! `CountItemsInString`). The geometric transfer matrix itself
//! (`mri_gtmpvc`) is a later module; these two functions move there with it.
//!
//! FreeSurfer keeps the list in two arrays of 1,000 `int` (the program's `SrcReplace` and
//! `TrgReplace`, zeroed) and its length (`nReplace`): [`Replacements`] keeps them as they are,
//! slots past the length included, so that the slots a later step reads hold what FreeSurfer's
//! hold.
//!
//! FS-1108: `GTMloadReplacmentList` after other replacements counts the earlier ones twice (the
//! list then holds slots past what it read: 0 to 0, or what an earlier list left there), prints
//! the total as the number it read, and takes an empty file as a list. larmorx counts each once,
//! prints the number read and refuses an empty file; `reproduce` as FreeSurfer.

use larmorx_core::defects::FS_1108;
use larmorx_freesurfer_core::cscan::{Bytes, Scanf};
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::{Fatal, errno_of, strerror};
use larmorx_freesurfer_core::utils::{count_items_in_string, resolve};

use super::options::{LIST_ROOM, Run};

/// The replacement list: the arrays (at least [`LIST_ROOM`] slots, more past it: FS-1109) and
/// `nReplace` (which `--gtm-default-seg-merge-choroid` lowers by 2 in `reproduce`).
#[derive(Clone, Debug)]
pub struct Replacements {
    pub src: Vec<i32>,
    pub trg: Vec<i32>,
    pub n: i32,
}

impl Default for Replacements {
    fn default() -> Self {
        Replacements {
            src: vec![0; LIST_ROOM],
            trg: vec![0; LIST_ROOM],
            n: 0,
        }
    }
}

impl Replacements {
    /// At least `len` slots (new ones 0).
    pub fn ensure(&mut self, len: usize) {
        if self.src.len() < len {
            self.src.resize(len, 0);
            self.trg.resize(len, 0);
        }
    }
    /// The slot `at` set to `src` -> `trg`.
    fn set(&mut self, at: usize, src: i32, trg: i32) {
        self.ensure(at + 1);
        self.src[at] = src;
        self.trg[at] = trg;
    }
    /// The first `n` slots (`n` below 0: none).
    pub fn pairs(&self) -> (&[i32], &[i32]) {
        let n = self.n.max(0) as usize;
        (&self.src[..n], &self.trg[..n])
    }
}

/// The default list's pairs, in FreeSurfer's order: the ventricular CSF into 24, the corpus
/// callosum's parts into 192, the cortex's unknowns, the optic chiasm, the vessels and the
/// non-WM hypointensities into 0.
const DEFAULT_LIST: [(i32, i32); 19] = [
    (4, 24),
    (5, 24),
    (14, 24),
    (15, 24),
    (72, 24),
    (43, 24),
    (44, 24),
    (251, 192),
    (252, 192),
    (253, 192),
    (254, 192),
    (255, 192),
    (1000, 0),
    (2000, 0),
    (85, 0),
    (30, 0),
    (62, 0),
    (80, 0),
    // the choroid plexus (31 and 63 into 24) is left out (FreeSurfer's comment)
    (0, 0),
];

/// `GTMdefaultSegReplacmentList(&nReplace, ReplaceThis, WithThat)`: the 18 default pairs added
/// after the list's length.
pub fn gtm_default_seg_replacment_list(rp: &mut Replacements) {
    let mut nlist = rp.n.max(0) as usize;
    for &(s, t) in &DEFAULT_LIST[..18] {
        rp.set(nlist, s, t);
        nlist += 1;
    }
    rp.n = nlist as i32;
}

/// The lines `fgets(tmpstr, 1000, fp)` gives: at most 999 bytes each, a line longer than that
/// read in pieces.
pub fn fgets_lines(bytes: &[u8], size: usize) -> Vec<&[u8]> {
    let mut lines = Vec::new();
    let mut at = 0;
    while at < bytes.len() {
        let mut end = at;
        while end < bytes.len() && end - at < size - 1 {
            end += 1;
            if bytes[end - 1] == b'\n' {
                break;
            }
        }
        lines.push(&bytes[at..end]);
        at = end;
    }
    lines
}

/// `GTMloadReplacmentList(fname, &nReplace, ReplaceThis, WithThat)`: each line not starting with
/// `#` must hold two numbers (`ERROR: line <n> in <file> has the wrong format (has <k> elements,
/// expecting 2)`, 1); then `Read in <n> replacement items from <file>`, 0. A file that does not
/// open: `ERROR: could not open <file> <errno>` and its text, 1; none read: `ERROR: could not
/// find any replacement items in <file>`, 1. FS-1108 for a list already holding replacements.
pub fn gtm_load_replacment_list(
    g: &mut Globals,
    r: &mut Run,
    fname: &str,
    rp: &mut Replacements,
) -> Result<i32, Fatal> {
    let bytes = match std::fs::read(resolve(r.dir, fname)) {
        Ok(b) => b,
        Err(e) => {
            let en = errno_of(&e).unwrap_or(0);
            // a folder opens (fopen "r") and reads nothing: fgets fails at once
            if en != 21 {
                g.errno = en;
                let _ = writeln!(r.out, "ERROR: could not open {fname} {en}");
                let _ = writeln!(r.out, "{}", strerror(en));
                return Ok(1);
            }
            Vec::new()
        }
    };
    let start = rp.n;
    let mut nlist = start.max(0) as usize;
    for (nth, line) in fgets_lines(&bytes, 1000).into_iter().enumerate() {
        if line.first() == Some(&b'#') {
            continue;
        }
        let k = count_items_in_string(line);
        if k != 2 {
            let _ = writeln!(
                r.out,
                "ERROR: line {nth} in {fname} has the wrong format (has {k} elements, expecting 2)"
            );
            return Ok(1);
        }
        // sscanf(tmpstr, "%d %d", ...): the second only when the first was read
        let (a, b) = {
            let mut st = Bytes::new(line);
            let mut sc = Scanf::new(&mut st);
            let a = sc.d(&mut g.errno, true);
            sc.text(&mut g.errno, b" ");
            let b = sc.d(&mut g.errno, true);
            sc.finish(&mut g.errno);
            (a, b)
        };
        rp.ensure(nlist + 1);
        if let Some(a) = a {
            rp.src[nlist] = a;
        }
        if let Some(b) = b {
            rp.trg[nlist] = b;
        }
        nlist += 1;
    }
    let read = nlist as i32 - start.max(0);
    if start > 0 {
        g.notices.add(
            &FS_1108,
            format!(
                "mri_segstats --replace-file after {start} replacements: each counted once (FreeSurfer counts the earlier ones twice)"
            ),
        );
    }
    let reproduce = g.notices.reproduce();
    let (none, shown) = if reproduce || start <= 0 {
        (nlist == 0, nlist as i32)
    } else {
        (read == 0, read)
    };
    if none {
        let _ = writeln!(
            r.out,
            "ERROR: could not find any replacement items in {fname}"
        );
        return Ok(1);
    }
    let _ = writeln!(r.out, "Read in {shown} replacement items from {fname}");
    if reproduce {
        rp.n = rp.n.wrapping_add(nlist as i32);
        rp.ensure(rp.n.max(0) as usize);
    } else {
        rp.n = nlist as i32;
    }
    Ok(0)
}
