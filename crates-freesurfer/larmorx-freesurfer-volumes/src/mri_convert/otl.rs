// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `mriio.cpp`'s outline reader `MRIreadOtl` (7144) with `read_otl_file` (6556) and the
//! unknown-label helpers (6521-6554), on `cma.cpp`'s outline fields (48-366:
//! `CMAoutlineFieldAlloc`, `CMAfreeOutlineField`, `CMAclearFillField`, `CMAfill`,
//! `CMAclaimPoints`, `CMAvalueClaims`, `CMAvalueAllClaims`, `CMAaddWeightedTotals`,
//! `CMAtotalClaims`, `CMAassignLabels`, `CMAzeroOutlines`; queue item 3.2-06, track A's relay
//! 48): `mri_convert`'s `.otl` input, a series of GDF outline files `<stem><n>.otl` (`n` from 1
//! to the slice count), each a slice of labels filled from its outlines.
//!
//! The port's own forms, with FreeSurfer's results: `CMAfill`'s recursion is an explicit stack
//! that visits the places in the recursion's order and measures its depth (FS-1144);
//! `CMAtotalClaims`' table of 5,308 sums per place is kept sparse (the labels the place's nine
//! neighbours claim, in the order FreeSurfer adds them; the others are 0 and never win, as every
//! sum is at least 0), and its last entry, which FreeSurfer never clears (FS-1055), is followed
//! on its own.
//!
//! Defects (`docs/findings/freesurfer-convert.md` Part 4): FS-1054, FS-1055, FS-1056, FS-1057,
//! FS-1058, FS-1059, FS-1060, FS-1141, FS-1142, FS-1143, FS-1144, FS-1145, FS-249.

use std::io::Write;

use larmorx_core::defects::{
    FS_249, FS_1054, FS_1055, FS_1056, FS_1057, FS_1058, FS_1059, FS_1060, FS_1141, FS_1142,
    FS_1143, FS_1144, FS_1145,
};
use larmorx_freesurfer_core::alloc::MACHINE_BYTES;
use larmorx_freesurfer_core::cscan::{Bytes, Scanf};
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::{
    self, ERROR_BADFILE, ERROR_BADPARM, ERROR_NOMEMORY, ERROR_UNSUPPORTED, Fatal, NO_ERROR,
};
use larmorx_freesurfer_core::utils::file_exists;
use larmorx_freesurfer_io::cfile::CFile;
use larmorx_freesurfer_io::colortab::{
    ctab_copy_name, ctab_get_number_of_total_entries, ctab_read_ascii,
};
use larmorx_freesurfer_io::mriio::{MRI_CORONAL, set_direction_cosine};
use larmorx_freesurfer_mri::ctab::ColorTable;
use larmorx_freesurfer_mri::mri::{MRI_SHORT, Mri, mri_alloc, mri_alloc_header};
use larmorx_freesurfer_volume::cma::MAX_CMA_LABEL;

use super::options::Run;

/// `READ_OTL_READ_VOLUME_FLAG`: read the outlines (otherwise only the header).
pub const READ_OTL_READ_VOLUME_FLAG: i32 = 0x01;
/// `READ_OTL_FILL_FLAG`: fill the outlines.
pub const READ_OTL_FILL_FLAG: i32 = 0x02;
/// `READ_OTL_TRANSLATE_LABELS_FLAG`: the labels' names translated to the colour table's.
pub const READ_OTL_TRANSLATE_LABELS_FLAG: i32 = 0x04;
/// `READ_OTL_ZERO_OUTLINES_FLAG`: the outlines themselves set to 0.
pub const READ_OTL_ZERO_OUTLINES_FLAG: i32 = 0x08;

/// `STRLEN`: the line, label and name buffers.
const STRLEN: usize = 1024;
/// `MAX_UNKNOWN_LABELS`: the unknown labels listed at most.
const MAX_UNKNOWN_LABELS: usize = 100;
/// `MAX_OUTLINE_CLAIMS`: the claims a place keeps at most.
const MAX_OUTLINE_CLAIMS: usize = 10;
/// `CMA_FILL_OUTLINE`, `CMA_FILL_INTERIOR`.
const CMA_FILL_OUTLINE: i16 = 1;
const CMA_FILL_INTERIOR: i16 = 2;
/// `OTL_CLAIM_WEIGHT_CENTER` and `OTL_CLAIM_WEIGHT_SIDE` (the diagonal neighbours get the side
/// weight too: FS-1059), as the `float` parameter takes them.
const WEIGHT_CENTER: f32 = 1.0;
const WEIGHT_SIDE: f32 = 0.5;
/// `OTL_CLAIM_WEIGHT_DIAGONAL`, which FreeSurfer defines and does not use (FS-1059).
const WEIGHT_DIAGONAL: f32 = 0.707;
/// The bytes of one level of `CMAfill`'s recursion in the official build (its return address
/// and five saved registers).
const FILL_FRAME_BYTES: u64 = 48;
/// FS-1144's notice: a recursion deeper than this (7.5 MiB of a default 8 MiB stack, the rest
/// taken by the program's own frames) overflows FreeSurfer's stack.
const FILL_STACK_BYTES: u64 = 15 << 19;
/// FS-1143's notice: slice files opened beyond this pass a default limit of 1,024 open files.
const OPEN_FILES: usize = 1021;
/// `sizeof(CMAoutlineClaim)` plus a `short` and an `unsigned char` a place (FS-1145).
const FIELD_PLACE_BYTES: u64 = 72 + 2 + 1;

/// `CMAoutlineClaim`.
#[derive(Clone, Copy, Default)]
struct Claim {
    n_claims: i32,
    interior_claim_flag: i32,
    claim_labels: [i16; MAX_OUTLINE_CLAIMS],
    claim_values: [f32; MAX_OUTLINE_CLAIMS],
    no_label_claim: f32,
}

/// `CMAoutlineField`: `width` x `height` places, each row `y` of the three fields at
/// `y * width`.
struct Field {
    width: i32,
    height: i32,
    claim: Vec<Claim>,
    fill: Vec<i16>,
    outline_points: Vec<u8>,
}

/// The outcome of [`Field::total_claims`] over a slice, for the notices.
#[derive(Default)]
struct Scan {
    /// Places whose label differs between FreeSurfer's never-reset entry 5307 and a fresh one
    /// (FS-1055).
    differ_5307: u64,
    /// Places where entry 5307 was claimed.
    claimed_5307: u64,
    /// Places whose label differs with the diagonal weight FreeSurfer defines (FS-1059).
    differ_diag: u64,
}

impl Field {
    /// `CMAoutlineFieldAlloc(width, height)`: `None` after its message where the allocation fails
    /// (FS-1145: past the machine's memory).
    fn alloc(g: &mut Globals, err: &mut dyn Write, width: i32, height: i32) -> Option<Field> {
        let (w, h) = (i64::from(width.max(0)), i64::from(height.max(0)));
        let bytes = (w as u64)
            .saturating_mul(h as u64)
            .saturating_mul(FIELD_PLACE_BYTES);
        if width < 0 || height < 0 || bytes > MACHINE_BYTES {
            g.notices.add(
                &FS_1145,
                format!(
                    "MRIreadOtl: an outline field of {width} x {height} places ({bytes} bytes): FreeSurfer's outcome depends on the machine's memory; refused as by a machine without that memory"
                ),
            );
            g.errno = larmorx_freesurfer_core::alloc::ENOMEM;
            log::error_printf(
                g,
                err,
                ERROR_NOMEMORY,
                "CMAoutlineFieldAlloc(): error allocating claim field",
            );
            return None;
        }
        let n = (w * h) as usize;
        Some(Field {
            width,
            height,
            claim: vec![Claim::default(); n],
            fill: vec![0; n],
            outline_points: vec![0; n],
        })
    }

    fn at(&self, x: i32, y: i32) -> usize {
        y as usize * self.width as usize + x as usize
    }

    /// `CMAclearFillField`.
    fn clear_fill(&mut self) {
        self.fill.fill(0);
    }

    /// `CMAfill(field, seed_x, seed_y)`: the zero places 4-connected to the seed set to
    /// `CMA_FILL_INTERIOR`, in the recursion's order (right, left, down, up; the coordinates are
    /// `short`s); the deepest number of places filling at once.
    fn fill_from(&mut self, seed_x: i16, seed_y: i16) -> u64 {
        let open = |f: &Field, x: i16, y: i16| -> bool {
            x >= 0
                && i32::from(x) < f.width
                && y >= 0
                && i32::from(y) < f.height
                && f.fill[f.at(i32::from(x), i32::from(y))] == 0
        };
        if !open(self, seed_x, seed_y) {
            return 0;
        }
        let i = self.at(i32::from(seed_x), i32::from(seed_y));
        self.fill[i] = CMA_FILL_INTERIOR;
        let mut stack: Vec<(i16, i16, u8)> = vec![(seed_x, seed_y, 0)];
        let mut deepest = 1u64;
        while let Some(top) = stack.last_mut() {
            let (x, y, next) = *top;
            if next == 4 {
                stack.pop();
                continue;
            }
            top.2 += 1;
            let (nx, ny) = match next {
                0 => (x.wrapping_add(1), y),
                1 => (x.wrapping_sub(1), y),
                2 => (x, y.wrapping_add(1)),
                _ => (x, y.wrapping_sub(1)),
            };
            if open(self, nx, ny) {
                let i = self.at(i32::from(nx), i32::from(ny));
                self.fill[i] = CMA_FILL_INTERIOR;
                stack.push((nx, ny, 0));
                deepest = deepest.max(stack.len() as u64);
            }
        }
        deepest
    }

    /// `CMAclaimPoints(field, label, points, n_points, seed_x, seed_y)`: `NO_ERROR`, or
    /// `ERROR_BADPARM` after its message (the outline's points before a bad one stay marked).
    #[allow(clippy::too_many_arguments)]
    fn claim_points(
        &mut self,
        g: &mut Globals,
        err: &mut dyn Write,
        label: i16,
        points: &[i16],
        seed_x: i16,
        seed_y: i16,
        slice: i32,
    ) -> i32 {
        if label < 0 || i32::from(label) > MAX_CMA_LABEL {
            log::error_printf(
                g,
                err,
                ERROR_BADPARM,
                &format!(
                    "CMAclaimPoints(): label out of range (label is {label}, MAX_CMA_LABEL is {MAX_CMA_LABEL})"
                ),
            );
            return ERROR_BADPARM;
        }
        if seed_x < 0 || i32::from(seed_x) >= self.width {
            log::error_printf(
                g,
                err,
                ERROR_BADPARM,
                &format!(
                    "CMAclaimPoints(): seed point out of range (seed_x = {seed_x}, field width = {})",
                    self.width
                ),
            );
            return ERROR_BADPARM;
        }
        if seed_y < 0 || i32::from(seed_y) >= self.height {
            log::error_printf(
                g,
                err,
                ERROR_BADPARM,
                &format!(
                    "CMAclaimPoints(): seed point out of range (seed_y = {seed_y}, field height = {})",
                    self.height
                ),
            );
            return ERROR_BADPARM;
        }
        self.clear_fill();
        for &[x, y] in points.as_chunks::<2>().0 {
            if x < 0 || i32::from(x) >= self.width {
                log::error_printf(
                    g,
                    err,
                    ERROR_BADPARM,
                    "CMAclaimPoints(): outline point out of range (x)",
                );
                return ERROR_BADPARM;
            }
            if y < 0 || i32::from(y) >= self.height {
                log::error_printf(
                    g,
                    err,
                    ERROR_BADPARM,
                    "CMAclaimPoints(): outline point out of range (y)",
                );
                return ERROR_BADPARM;
            }
            let i = self.at(i32::from(x), i32::from(y));
            self.fill[i] = CMA_FILL_OUTLINE;
            self.outline_points[i] = 1;
        }
        let deepest = self.fill_from(seed_x, seed_y);
        let bytes = (deepest + 1) * FILL_FRAME_BYTES;
        if bytes > FILL_STACK_BYTES {
            g.notices.add(
                &FS_1144,
                format!(
                    "MRIreadOtl: slice {slice}'s fill of label {label} from ({seed_x}, {seed_y}) recurses {deepest} places deep ({bytes} bytes of stack in FreeSurfer, which a default 8 MiB stack does not hold: a crash there); filled"
                ),
            );
        }
        for c in 0..self.claim.len() {
            match self.fill[c] {
                CMA_FILL_INTERIOR => {
                    let cl = &mut self.claim[c];
                    cl.n_claims = 1;
                    cl.interior_claim_flag = 1;
                    cl.claim_labels[0] = label;
                }
                CMA_FILL_OUTLINE => {
                    let cl = &mut self.claim[c];
                    if (cl.n_claims as usize) < MAX_OUTLINE_CLAIMS {
                        cl.claim_labels[cl.n_claims as usize] = label;
                        cl.n_claims += 1;
                        cl.interior_claim_flag = 0;
                    }
                }
                _ => {}
            }
        }
        NO_ERROR
    }

    /// `CMAvalueAllClaims` (`CMAvalueClaims` at every place).
    fn value_all_claims(&mut self) {
        for c in &mut self.claim {
            c.claim_values = [0.0; MAX_OUTLINE_CLAIMS];
            if c.n_claims == 0 {
                c.no_label_claim = 0.5;
            } else if c.n_claims == 1 {
                if c.interior_claim_flag == 1 {
                    c.claim_values[0] = 1.0;
                } else {
                    c.claim_values[0] = 0.5;
                    c.no_label_claim = 0.5;
                }
            } else {
                // `1.0 / (float)n` in double, then a float
                let ct = (1.0f64 / f64::from(c.n_claims as f32)) as f32;
                for v in &mut c.claim_values[..c.n_claims as usize] {
                    *v = ct;
                }
            }
        }
    }

    /// The weighted sums of the claims of place (`x`, `y`) and its eight neighbours
    /// (`CMAaddWeightedTotals` with `weight` for the place, the side weight for its side
    /// neighbours and `diag` for its diagonal ones): the label below entry 5307 with the largest
    /// sum (the smallest on a tie, 0 for no label) and that sum; entry 5307's terms in the order
    /// FreeSurfer adds them into `c5307`, their count the third value.
    fn weighed(&self, x: i32, y: i32, diag: f32, c5307: &mut [f32; 90]) -> (i16, f32, usize) {
        // (label, sum) in the order the labels are first met; entry 0 first
        let mut sums = [(0i16, 0f32); 1 + 9 * MAX_OUTLINE_CLAIMS];
        let mut n = 1usize;
        let mut n5307 = 0usize;
        let (w, h) = (self.width, self.height);
        let mut add = |c: &Claim, weight: f32| {
            for k in 0..c.n_claims as usize {
                let label = c.claim_labels[k];
                let v = weight * c.claim_values[k];
                if i32::from(label) == MAX_CMA_LABEL {
                    c5307[n5307] = v;
                    n5307 += 1;
                    continue;
                }
                match sums[..n].iter_mut().find(|s| s.0 == label) {
                    Some(s) => s.1 += v,
                    None => {
                        sums[n] = (label, v);
                        n += 1;
                    }
                }
            }
            sums[0].1 += weight * c.no_label_claim;
        };
        let claim = |cx: i32, cy: i32| &self.claim[cy as usize * w as usize + cx as usize];
        add(claim(x, y), WEIGHT_CENTER);
        if x >= 1 {
            add(claim(x - 1, y), WEIGHT_SIDE);
        }
        if x + 1 < w {
            add(claim(x + 1, y), WEIGHT_SIDE);
        }
        if y >= 1 {
            add(claim(x, y - 1), WEIGHT_SIDE);
        }
        if y + 1 < h {
            add(claim(x, y + 1), WEIGHT_SIDE);
        }
        if x >= 1 && y >= 1 {
            add(claim(x - 1, y - 1), diag);
        }
        if x >= 1 && y + 1 < h {
            add(claim(x - 1, y + 1), diag);
        }
        if x + 1 < w && y >= 1 {
            add(claim(x + 1, y - 1), diag);
        }
        if x + 1 < w && y + 1 < h {
            add(claim(x + 1, y + 1), diag);
        }
        // the labels in increasing order (the untouched ones are 0, never above a sum: every
        // sum is at least 0)
        sums[1..n].sort_by_key(|s| s.0);
        let (mut best_claim, mut best_index) = (sums[0].1, 0i16);
        for &(label, sum) in &sums[1..n] {
            if sum > best_claim {
                best_claim = sum;
                best_index = label;
            }
        }
        (best_index, best_claim, n5307)
    }

    /// `CMAtotalClaims(field, x, y)`: the label with the largest weighted claim at (`x`, `y`).
    /// `acc` is FreeSurfer's entry 5307, never reset (FS-1055): a fresh entry decides the
    /// default's label, `acc` the label under `reproduce`. The diagonal weight FreeSurfer
    /// defines and does not use (FS-1059) is tried too, for the notice.
    fn total_claims(&self, x: i32, y: i32, acc: &mut f32, reproduce: bool, scan: &mut Scan) -> i16 {
        // a place whose nine places hold no claim: label 0 scores 0.5 for the place and 0.25 for
        // each neighbour inside the field (exact sums), every other label 0, with either weight
        let (w, h) = (self.width, self.height);
        let mut inside = 0u8;
        let mut claimed = false;
        for dy in -1..=1 {
            for dx in -1..=1 {
                let (cx, cy) = (x + dx, y + dy);
                if cx >= 0 && cx < w && cy >= 0 && cy < h {
                    inside += 1;
                    claimed |= self.claim[self.at(cx, cy)].n_claims != 0;
                }
            }
        }
        if !claimed {
            let best_claim = 0.5 + 0.25 * f32::from(inside - 1);
            let kept = if *acc > best_claim {
                MAX_CMA_LABEL as i16
            } else {
                0
            };
            if kept != 0 {
                scan.differ_5307 += 1;
            }
            return if reproduce { kept } else { 0 };
        }
        let mut c5307 = [0f32; 90];
        let (best_index, best_claim, n5307) = self.weighed(x, y, WEIGHT_SIDE, &mut c5307);
        let mut fresh_5307 = 0f32;
        for &v in &c5307[..n5307] {
            fresh_5307 += v;
            *acc += v;
        }
        let fresh = if fresh_5307 > best_claim {
            MAX_CMA_LABEL as i16
        } else {
            best_index
        };
        let kept = if *acc > best_claim {
            MAX_CMA_LABEL as i16
        } else {
            best_index
        };
        if n5307 > 0 {
            scan.claimed_5307 += 1;
        }
        if fresh != kept {
            scan.differ_5307 += 1;
        }
        let (mi, mc, m5307) = self.weighed(x, y, WEIGHT_DIAGONAL, &mut c5307);
        let meant_5307 = c5307[..m5307].iter().fold(0f32, |s, &v| s + v);
        let meant = if meant_5307 > mc {
            MAX_CMA_LABEL as i16
        } else {
            mi
        };
        if meant != fresh {
            scan.differ_diag += 1;
        }
        if reproduce { kept } else { fresh }
    }

    /// `CMAassignLabels`: every place's label from the claims (columns outside, rows inside: the
    /// order FS-1055's sum follows).
    fn assign_labels(&mut self, reproduce: bool) -> Scan {
        self.clear_fill();
        self.value_all_claims();
        let mut scan = Scan::default();
        // FS-1055: FreeSurfer's entry 5307 before the first place (memory never written)
        let mut acc = 0f32;
        let mut labels = vec![0i16; self.fill.len()];
        for x in 0..self.width {
            for y in 0..self.height {
                labels[self.at(x, y)] = self.total_claims(x, y, &mut acc, reproduce, &mut scan);
            }
        }
        self.fill = labels;
        scan
    }

    /// `CMAzeroOutlines`.
    fn zero_outlines(&mut self) {
        for (f, &o) in self.fill.iter_mut().zip(&self.outline_points) {
            if o != 0 {
                *f = 0;
            }
        }
    }
}

/// `read_otl_file`'s line buffer (`char line[STRLEN]`): each `fgets` writes its bytes and a NUL
/// and leaves the rest as earlier lines left it (read as C strings from a place: FreeSurfer reads
/// a short line's later bytes from there). Zero before the first line (FreeSurfer's memory never
/// written, read only by the check of a file that gave no line, FS-1054).
struct Line([u8; STRLEN]);

impl Line {
    /// `if (!fgets(line, STRLEN, fp) && ferror(fp)) ErrorPrintf(...)`.
    fn gets(&mut self, g: &mut Globals, err: &mut dyn Write, fp: &mut CFile) {
        match fp.fgets(STRLEN) {
            Some(b) => {
                self.0[..b.len()].copy_from_slice(&b);
                self.0[b.len()] = 0;
            }
            None => {
                if fp.ferror() {
                    if let Some(e) = fp.read_errno() {
                        g.errno = e;
                    }
                    log::error_printf(
                        g,
                        err,
                        ERROR_BADFILE,
                        "read_otl_file(): could not read file",
                    );
                }
            }
        }
    }

    /// The C string from byte `at`.
    fn cstr(&self, at: usize) -> &[u8] {
        c_str(&self.0[at.min(STRLEN)..])
    }

    /// `strncmp(line, lit, lit.len()) == 0`.
    fn is(&self, lit: &[u8]) -> bool {
        self.cstr(0).starts_with(lit)
    }

    /// `sscanf(line, "%*s %d", &v)`: `v` when it was read.
    fn word_int(&self, g: &mut Globals) -> Option<i32> {
        let mut b = Bytes::new(&self.0);
        let mut s = Scanf::new(&mut b);
        s.s(&mut g.errno, usize::MAX, false);
        s.text(&mut g.errno, b" ");
        let v = s.d(&mut g.errno, true);
        s.finish(&mut g.errno);
        v
    }

    /// `sscanf(line, "%*s %d %d", &a, &b)`.
    fn word_int_int(&self, g: &mut Globals) -> (Option<i32>, Option<i32>) {
        let mut b = Bytes::new(&self.0);
        let mut s = Scanf::new(&mut b);
        s.s(&mut g.errno, usize::MAX, false);
        s.text(&mut g.errno, b" ");
        let x = s.d(&mut g.errno, true);
        s.text(&mut g.errno, b" ");
        let y = s.d(&mut g.errno, true);
        s.finish(&mut g.errno);
        (x, y)
    }

    /// `sscanf(line, "%hd %hd", &a, &b)` (each `(short)` of the `long` read).
    fn two_shorts(&self, g: &mut Globals) -> (Option<i16>, Option<i16>) {
        let mut b = Bytes::new(&self.0);
        let mut s = Scanf::new(&mut b);
        let x = s.d(&mut g.errno, true).map(|v| v as i16);
        s.text(&mut g.errno, b" ");
        let y = s.d(&mut g.errno, true).map(|v| v as i16);
        s.finish(&mut g.errno);
        (x, y)
    }
}

/// The bytes before the first NUL.
fn c_str(b: &[u8]) -> &[u8] {
    &b[..b.iter().position(|&c| c == 0).unwrap_or(b.len())]
}

/// A string copied from `&line[at]`, cut at its last newline (`strcpy` then `strrchr` + NUL).
fn header_text(line: &Line, at: usize) -> Vec<u8> {
    let mut t = line.cstr(at).to_vec();
    if let Some(p) = t.iter().rposition(|&c| c == b'\n') {
        t.truncate(p);
    }
    t
}

/// `strstr(hay, needle)`'s place.
fn find(hay: &[u8], needle: &[u8]) -> Option<usize> {
    hay.windows(needle.len()).position(|w| w == needle)
}

/// `MRIreadOtl`'s list of unknown labels (`MAX_UNKNOWN_LABELS` at most, each once).
#[derive(Default)]
struct Unknown(Vec<Vec<u8>>);

impl Unknown {
    /// `register_unknown_label(label)`.
    fn register(&mut self, label: &[u8]) {
        if self.0.len() == MAX_UNKNOWN_LABELS || self.0.iter().any(|l| l == label) {
            return;
        }
        self.0.push(label.to_vec());
    }
}

/// One slice's file: what the reader needs besides the file.
struct Slice<'a> {
    slice: i32,
    ctab: &'a ColorTable,
    translate: bool,
    zero_outlines: bool,
}

/// An error of `read_otl_file` (`errno = 0; ErrorReturn(code, (code, message))`).
fn fail(g: &mut Globals, err: &mut dyn Write, code: i32, message: &[u8]) -> i32 {
    g.errno = 0;
    log::error_printf_bytes(g, err, code, message);
    code
}

/// `read_otl_file(fp, mri, slice, ctab, fill_flag, translate_label_flag, zero_outlines_flag)`:
/// `NO_ERROR` with the slice's labels in `mri`'s slice `slice - 1`, or the error after its
/// message.
fn read_otl_file(
    g: &mut Globals,
    r: &mut Run,
    fp: &mut CFile,
    mri: &mut Mri,
    s: &Slice,
    unknown: &mut Unknown,
) -> i32 {
    let slice = s.slice;
    let err = &mut *r.err;
    let mut line = Line([0; STRLEN]);
    let mut n_outlines = -1i32;
    line.gets(g, err, fp);
    if !line.is(b"GDF FILE VERSIO") {
        g.notices.add(
            &FS_1054,
            format!(
                "MRIreadOtl: slice {slice}'s file does not start with GDF FILE VERSION: FreeSurfer's message prints the slice number as a string and crashes; the message printed with the number"
            ),
        );
        return fail(
            g,
            err,
            ERROR_BADFILE,
            format!("otl slice {slice} does not appear to be a GDF file").as_bytes(),
        );
    }
    let mut main_header_flag = false;
    while !main_header_flag {
        if fp.feof() {
            return fail(
                g,
                err,
                ERROR_BADFILE,
                format!("premature EOF () in otl file {slice}").as_bytes(),
            );
        }
        line.gets(g, err, fp);
        if line.is(b"START MAIN HEADER") {
            main_header_flag = true;
        }
    }
    let mut n_cols = -1i32;
    let mut global_type: Vec<u8> = Vec::new();
    while main_header_flag {
        if fp.feof() {
            return fail(
                g,
                err,
                ERROR_BADFILE,
                format!("premature EOF (in main header) in otl file {slice}").as_bytes(),
            );
        }
        line.gets(g, err, fp);
        if line.is(b"END MAIN HEADER") {
            main_header_flag = false;
        }
        if line.is(b"ONUM ")
            && let Some(v) = line.word_int(g)
        {
            n_outlines = v;
        }
        if line.is(b"COL_NUM")
            && let Some(v) = line.word_int(g)
        {
            n_cols = v;
        }
        if line.is(b"TYPE") {
            global_type = header_text(&line, 5);
        }
    }
    if n_outlines == -1 {
        return fail(
            g,
            err,
            ERROR_BADPARM,
            format!("bad or undefined ONUM in otl file {slice}").as_bytes(),
        );
    }
    let Some(mut of) = Field::alloc(
        g,
        err,
        mri.vg.width.wrapping_mul(2),
        mri.vg.height.wrapping_mul(2),
    ) else {
        return ERROR_NOMEMORY;
    };
    // FS-1060: the warning's flag as FreeSurfer keeps it, and as it is meant
    let (mut internal_fs, mut internal_meant) = (false, false);
    let reproduce = g.notices.reproduce();
    for i in 0..n_outlines.max(0) {
        if fp.feof() {
            return fail(
                g,
                err,
                ERROR_BADFILE,
                format!("premature EOF (ready for next outline) in otl file {slice}").as_bytes(),
            );
        }
        let mut gdf_header_flag = false;
        while !gdf_header_flag {
            if fp.feof() {
                return fail(
                    g,
                    err,
                    ERROR_BADFILE,
                    format!("premature EOF (searching for gdf header) in otl file {slice}")
                        .as_bytes(),
                );
            }
            line.gets(g, err, fp);
            if line.is(b"START GDF HEADER") {
                gdf_header_flag = true;
            }
        }
        let mut n_rows = -1i32;
        let (mut seed_x, mut seed_y) = (-1i32, -1i32);
        let mut label: Vec<u8> = Vec::new();
        let mut otype: Vec<u8> = Vec::new();
        while gdf_header_flag {
            if fp.feof() {
                return fail(
                    g,
                    err,
                    ERROR_BADFILE,
                    format!("premature EOF (in gdf header) in otl file {slice}").as_bytes(),
                );
            }
            line.gets(g, err, fp);
            if line.is(b"END GDF HEADER") {
                gdf_header_flag = false;
            }
            if line.is(b"ROW_NUM")
                && let Some(v) = line.word_int(g)
            {
                n_rows = v;
            }
            if line.is(b"COL_NUM")
                && let Some(v) = line.word_int(g)
            {
                n_cols = v;
            }
            if line.is(b"TYPE") {
                otype = header_text(&line, 5);
            }
            if line.is(b"SEED") {
                let (x, y) = line.word_int_int(g);
                if let Some(x) = x {
                    seed_x = x;
                }
                if let Some(y) = y {
                    seed_y = y;
                }
            }
            if line.is(b"LABEL") {
                label = header_text(&line, 6);
                if s.translate {
                    // sprintf(at, "Cortex"): the rest of the name after the word is dropped
                    if let Some(p) = find(&label, b"Exterior") {
                        label.truncate(p);
                        label.extend_from_slice(b"Cortex");
                    } else if let Some(p) = find(&label, b"exterior") {
                        label.truncate(p);
                        label.extend_from_slice(b"cortex");
                    }
                }
                let words = [&b"Exterior"[..], b"exterior", b"Cortex", b"cortex"];
                let all = words.iter().all(|w| find(&label, w).is_some());
                let any = words.iter().any(|w| find(&label, w).is_some());
                // FreeSurfer: a label without all four words warns after one with them
                let warn_fs = !all && internal_fs;
                internal_fs = all;
                // meant: a label with one of them warns after another label
                let warn_meant = any && internal_meant;
                internal_meant = !any;
                if warn_fs != warn_meant {
                    g.notices.add(
                        &FS_1060,
                        format!(
                            "MRIreadOtl: slice {slice}'s label \"{}\": FreeSurfer's warning of an exterior label after other labels is {} here, where it is {}meant",
                            String::from_utf8_lossy(&label),
                            if warn_fs { "printed" } else { "not printed" },
                            if warn_meant { "" } else { "not " }
                        ),
                    );
                }
                if if reproduce { warn_fs } else { warn_meant } {
                    let mut m = b"WARNING: label \"".to_vec();
                    m.extend_from_slice(&label);
                    m.extend_from_slice(
                        format!("\" following non-exterior labels in slice {slice}\n").as_bytes(),
                    );
                    let _ = r.out.write_all(&m);
                }
            }
        }
        if n_rows < 0 {
            return fail(
                g,
                err,
                ERROR_BADPARM,
                format!("bad or undefined ROW_NUM in otl file {slice}").as_bytes(),
            );
        }
        if n_cols != 2 {
            return fail(
                g,
                err,
                ERROR_BADPARM,
                format!("bad or undefined COL_NUM in otl file {slice}").as_bytes(),
            );
        }
        let empty_label = label.is_empty();
        if empty_label {
            g.errno = 0;
            log::error_printf(
                g,
                err,
                ERROR_BADPARM,
                &format!("empty LABEL in otl file {slice} (outline {i})"),
            );
        }
        if !(0..512).contains(&seed_x) || !(0..512).contains(&seed_y) {
            return fail(
                g,
                err,
                ERROR_BADPARM,
                format!("bad or undefined SEED in otl file {slice}").as_bytes(),
            );
        }
        if otype.is_empty() {
            otype = global_type.clone();
        }
        let ascii = match otype.as_slice() {
            b"ascii short" => true,
            b"short" => false,
            b"" => {
                return fail(
                    g,
                    err,
                    ERROR_UNSUPPORTED,
                    format!("undefined TYPE in otl file {slice}").as_bytes(),
                );
            }
            t => {
                let mut m = b"unsupported TYPE \"".to_vec();
                m.extend_from_slice(t);
                m.extend_from_slice(format!("\" in otl file {slice}").as_bytes());
                return fail(g, err, ERROR_UNSUPPORTED, &m);
            }
        };
        loop {
            line.gets(g, err, fp);
            if fp.feof() {
                return fail(
                    g,
                    err,
                    ERROR_BADFILE,
                    format!("premature EOF (searching for points) in otl file {slice}").as_bytes(),
                );
            }
            if line.is(b"START POINTS") {
                break;
            }
        }
        // malloc(2 * n_rows * sizeof(short)): the int product wraps from 2^30 rows on, a size
        // no allocation gives
        if n_rows >= 1 << 30 {
            return fail(
                g,
                err,
                ERROR_NOMEMORY,
                format!("error allocating memory for points in otl file {slice}").as_bytes(),
            );
        }
        let mut points: Vec<i16> = Vec::new();
        if ascii {
            let mut unread = 0u64;
            for _ in 0..n_rows {
                line.gets(g, err, fp);
                if fp.feof() {
                    return fail(
                        g,
                        err,
                        ERROR_BADFILE,
                        format!("premature end of file while reading points from otl file {slice}")
                            .as_bytes(),
                    );
                }
                let (x, y) = line.two_shorts(g);
                unread += u64::from(x.is_none()) + u64::from(y.is_none());
                points.push(x.unwrap_or(0));
                points.push(y.unwrap_or(0));
            }
            if unread > 0 {
                g.notices.add(
                    &FS_1141,
                    format!(
                        "MRIreadOtl: slice {slice}'s outline {i} has {unread} coordinates its point lines do not hold: FreeSurfer's are memory never written; 0 here"
                    ),
                );
            }
        } else {
            let want = n_rows as usize * 4;
            let mut bytes = Vec::new();
            let mut chunk = vec![0u8; want.min(1 << 20)];
            while bytes.len() < want {
                let k = (want - bytes.len()).min(chunk.len());
                let got = fp.read_bytes(&mut chunk[..k]);
                bytes.extend_from_slice(&chunk[..got]);
                if got < k {
                    if fp.ferror()
                        && let Some(e) = fp.read_errno()
                    {
                        g.errno = e;
                    }
                    break;
                }
            }
            if bytes.len() / 2 != n_rows as usize * 2 {
                return fail(
                    g,
                    err,
                    ERROR_BADFILE,
                    format!("error reading points from otl file {slice}").as_bytes(),
                );
            }
            // big-endian shorts (swab)
            points = bytes
                .as_chunks::<2>()
                .0
                .iter()
                .map(|&b| i16::from_be_bytes(b))
                .collect();
        }
        line.gets(g, err, fp);
        if !line.is(b"END POINTS") {
            return fail(
                g,
                err,
                ERROR_BADFILE,
                format!(
                    "error after points (\"END POINTS\" expected but not found) in otl file {slice}"
                )
                .as_bytes(),
            );
        }
        if !empty_label {
            claim_outline(
                g,
                err,
                &mut of,
                s,
                &label,
                &points,
                (seed_x, seed_y),
                unknown,
            );
        }
    }
    let scan = of.assign_labels(reproduce);
    if scan.differ_diag > 0 {
        g.notices.add(
            &FS_1059,
            format!(
                "MRIreadOtl: slice {slice}: the diagonal neighbours weighed as side ones (0.5) give {} places of the outline field another label than the diagonal weight FreeSurfer defines (0.707); kept",
                scan.differ_diag
            ),
        );
    }
    if scan.differ_5307 > 0 {
        g.notices.add(
            &FS_1055,
            format!(
                "MRIreadOtl: slice {slice}: entry {MAX_CMA_LABEL} is claimed at {} places of the outline field; FreeSurfer's never-reset score of it changes the label of {} places{}",
                scan.claimed_5307,
                scan.differ_5307,
                if reproduce {
                    " (kept, from 0)"
                } else {
                    " (each place scored afresh here)"
                }
            ),
        );
    }
    if s.zero_outlines {
        of.zero_outlines();
    }
    write_slice(g, mri, &of, slice);
    NO_ERROR
}

/// The label lookup and the claims of one outline (`read_otl_file` after `END POINTS`).
#[allow(clippy::too_many_arguments)]
fn claim_outline(
    g: &mut Globals,
    err: &mut dyn Write,
    of: &mut Field,
    s: &Slice,
    label: &[u8],
    points: &[i16],
    seed: (i32, i32),
    unknown: &mut Unknown,
) {
    // spaces to dashes (the label holds no newline)
    let mut cmp: Vec<u8> = label
        .iter()
        .map(|&c| if c == b' ' { b'-' } else { c })
        .collect();
    let lower = cmp.to_ascii_lowercase();
    let alt: Vec<u8> = if lower.starts_with(b"left-") {
        cmp[5..].to_vec()
    } else if lower.starts_with(b"right-") {
        cmp[6..].to_vec()
    } else {
        lower
    };
    let lead = cmp.iter().take_while(|&&c| c == b'-').count();
    if lead < cmp.len() {
        cmp.drain(..lead);
    }
    if let Some(last) = cmp.iter().rposition(|&c| c != b'-') {
        cmp.truncate(last + 1);
    }
    let mut label_value = -1i16;
    let num_entries = ctab_get_number_of_total_entries(s.ctab);
    for j in 0..num_entries {
        if let Ok(name) = ctab_copy_name(g, err, s.ctab, j, STRLEN) {
            let name = c_str(&name);
            if name == cmp.as_slice() || name == alt.as_slice() {
                label_value = j as i16;
            }
        }
    }
    if label_value == -1 {
        unknown.register(label);
        return;
    }
    let outside = points
        .as_chunks::<2>()
        .0
        .iter()
        .filter(|p| !(0..512).contains(&p[0]) || !(0..512).contains(&p[1]))
        .count();
    if outside > 0 {
        g.notices.add(
            &FS_1056,
            format!(
                "MRIreadOtl: slice {}'s outline of label {label_value} has {outside} points outside 0 to 511, which FreeSurfer's source writes outside a 512 x 512 array (undefined behaviour; the official build leaves that never-read array out); not written here",
                s.slice
            ),
        );
    }
    of.claim_points(
        g,
        err,
        label_value,
        points,
        seed.0 as i16,
        seed.1 as i16,
        s.slice,
    );
}

/// `read_otl_file`'s end: the volume's slice `slice - 1` sampled from the field at whole steps of
/// `512 / width` (FS-1058; the place a step of 512 divided by the width would sample is tried for
/// the notice), places past the field 0 (FS-1057).
fn write_slice(g: &mut Globals, mri: &mut Mri, of: &Field, slice: i32) {
    let (width, height) = (mri.vg.width, mri.vg.height);
    let scale_x = (512 / width) as f32;
    let scale_y = (512 / height) as f32;
    let sample = |sx: i32, sy: i32, past: &mut u64| -> i16 {
        if sx < of.width && sy < of.height {
            of.fill[of.at(sx, sy)]
        } else {
            *past += 1;
            0
        }
    };
    let (mut past, mut past_meant, mut differ) = (0u64, 0u64, 0u64);
    let vox_per_row = mri.vox_per_row as usize;
    let base = (slice - 1) as usize * mri.vox_per_slice as usize;
    let Some(chunk) = mri.chunk.as_mut() else {
        return;
    };
    for i in 0..width {
        for j in 0..height {
            let sx = ((scale_x * i as f32).floor() as i32).clamp(0, 511);
            let sy = ((scale_y * j as f32).floor() as i32).clamp(0, 511);
            let v = sample(sx, sy, &mut past);
            let mx = ((512.0 * f64::from(i) / f64::from(width)).floor() as i32).clamp(0, 511);
            let my = ((512.0 * f64::from(j) / f64::from(height)).floor() as i32).clamp(0, 511);
            if sample(mx, my, &mut past_meant) != v {
                differ += 1;
            }
            let o = (base + j as usize * vox_per_row + i as usize) * 2;
            chunk[o..o + 2].copy_from_slice(&v.to_le_bytes());
        }
    }
    if past > 0 {
        g.notices.add(
            &FS_1057,
            format!(
                "MRIreadOtl: slice {slice}: {past} voxels of a {width} x {height} volume sample places past the {} x {} outline field (memory FreeSurfer does not own); 0 here",
                of.width, of.height
            ),
        );
    }
    if differ > 0 {
        g.notices.add(
            &FS_1058,
            format!(
                "MRIreadOtl: slice {slice}: a volume of {width} x {height} voxels samples the outlines' 512 x 512 space at whole steps of {scale_x} x {scale_y}, which give {differ} voxels another label than steps of 512 divided by the size; kept"
            ),
        );
    }
}

/// `MRIreadOtl(fname, width, height, slices, color_file_name, flags)`: the series
/// `<stem><n>.otl` (`<stem>` the name before the digits that end it before its last `.`; `n`
/// from 1 to `slices`) as a `width` x `height` x `slices` volume of shorts (512 for a width or
/// height of 0 or less), coronal, or its header only without [`READ_OTL_READ_VOLUME_FLAG`]; the
/// colour table names the labels. `None` after the error's message.
#[allow(clippy::too_many_arguments)]
pub fn mri_read_otl(
    g: &mut Globals,
    r: &mut Run,
    fname: &str,
    width: i32,
    height: i32,
    slices: i32,
    color_file_name: &str,
    flags: i32,
) -> Result<Option<Mri>, Fatal> {
    const WHO: &str = "MRIreadOtl";
    let read_volume = flags & READ_OTL_READ_VOLUME_FLAG != 0;
    let translate = flags & READ_OTL_TRANSLATE_LABELS_FLAG != 0;
    let zero_outlines = flags & READ_OTL_ZERO_OUTLINES_FLAG != 0;
    let mut unknown = Unknown::default();
    let width = if width <= 0 { 512 } else { width };
    let height = if height <= 0 { 512 } else { height };
    if fname.len() >= STRLEN {
        g.notices.add(
            &FS_249,
            format!(
                "{WHO}: a name of {} bytes used whole (FreeSurfer's buffer holds 1023)",
                fname.len()
            ),
        );
    }
    let bad_name = |g: &mut Globals, err: &mut dyn Write| {
        g.errno = 0;
        log::error_printf(
            g,
            err,
            ERROR_BADPARM,
            &format!("MRIreadOtl(): bad file name: {fname}"),
        );
    };
    let b = fname.as_bytes();
    let Some(dot) = b.iter().rposition(|&c| c == b'.') else {
        bad_name(g, r.err);
        return Ok(None);
    };
    let mut c = dot as i64 - 1;
    while c >= 0 && b[c as usize].is_ascii_digit() {
        c -= 1;
    }
    if c < 0 {
        bad_name(g, r.err);
        return Ok(None);
    }
    let prefix = &fname[..(c + 1) as usize];
    let name_of = |i: i32| format!("{prefix}{i}.otl");
    let mut one_file_exists = false;
    for i in 1..=slices.max(0) {
        if file_exists(r.dir, &name_of(i)) {
            one_file_exists = true;
        }
    }
    let first_name = if slices >= 1 {
        name_of(1)
    } else {
        String::new()
    };
    let last_name = if slices >= 1 {
        name_of(slices)
    } else {
        fname.to_owned()
    };
    if last_name.len() >= STRLEN {
        g.notices.add(
            &FS_249,
            format!(
                "{WHO}: a name of {} bytes used whole (FreeSurfer's buffer holds 1023)",
                last_name.len()
            ),
        );
    }
    if !one_file_exists {
        if slices < 1 {
            g.notices.add(
                &FS_1142,
                format!(
                    "{WHO}: a slice count of {slices}: FreeSurfer's message prints a first name it never wrote (memory contents); empty here"
                ),
            );
        }
        g.errno = 0;
        log::error_printf(
            g,
            r.err,
            ERROR_BADPARM,
            &format!("MRIreadOtl(): couldn't find any file between {first_name} and {last_name}"),
        );
        return Ok(None);
    }
    if !read_volume {
        let m = mri_alloc_header(g, r.out, r.err, width, height, slices, MRI_SHORT, 1)?;
        return Ok(Some(m));
    }
    let mut mri = mri_alloc(g, r.out, r.err, width, height, slices, MRI_SHORT)?;
    let Some(ctab) = ctab_read_ascii(g, r.out, r.err, r.dir, color_file_name) else {
        g.errno = 0;
        log::error_printf(
            g,
            r.err,
            ERROR_BADFILE,
            &format!("MRIreadOtl(): error reading color file {color_file_name}"),
        );
        return Ok(None);
    };
    let mut one_file_exists = false;
    let mut opened = 0usize;
    for i in 1..=slices {
        let name = name_of(i);
        let mut fp = match CFile::open(r.dir, &name, "r") {
            Ok(f) => f,
            Err(e) => {
                g.errno = e;
                continue;
            }
        };
        opened += 1;
        if opened == OPEN_FILES + 1 {
            g.notices.add(
                &FS_1143,
                format!(
                    "{WHO}: more than {OPEN_FILES} slice files, which FreeSurfer keeps open: past a limit of 1,024 open files its later slices are left empty; each read here"
                ),
            );
        }
        let s = Slice {
            slice: i,
            ctab: &ctab,
            translate,
            zero_outlines,
        };
        let rc = read_otl_file(g, r, &mut fp, &mut mri, &s, &mut unknown);
        // FreeSurfer never closes the file (FS-1143)
        fp.close();
        if rc != NO_ERROR {
            return Ok(None);
        }
        one_file_exists = true;
    }
    if !one_file_exists {
        g.errno = 0;
        log::error_printf(
            g,
            r.err,
            ERROR_BADFILE,
            &format!(
                "MRIreadOtl(): found at least one file between {first_name} and {last_name} but couldn't open it!"
            ),
        );
        return Ok(None);
    }
    if unknown.0.is_empty() {
        let _ = writeln!(r.out, "no unknown labels");
    } else {
        let _ = writeln!(r.out, "unknown labels:");
        for l in &unknown.0 {
            let _ = r.out.write_all(b"  ");
            let _ = r.out.write_all(l);
            let _ = r.out.write_all(b"\n");
        }
    }
    set_direction_cosine(g, r.err, &mut mri.vg, MRI_CORONAL);
    Ok(Some(mri))
}

#[cfg(test)]
mod tests {
    use super::*;
    use larmorx_core::defects::UpstreamDefects;

    fn field(w: i32, h: i32) -> Field {
        let mut g = Globals::new(UpstreamDefects::Fix);
        Field::alloc(&mut g, &mut Vec::new(), w, h).expect("a field")
    }

    #[test]
    fn fill_stops_at_the_outline_and_measures_its_depth() {
        let mut f = field(5, 4);
        // a vertical wall at x = 2
        for y in 0..4 {
            let i = f.at(2, y);
            f.fill[i] = CMA_FILL_OUTLINE;
        }
        let deepest = f.fill_from(0, 0);
        let filled: usize = f.fill.iter().filter(|&&v| v == CMA_FILL_INTERIOR).count();
        assert_eq!(filled, 8);
        // the chain right, down, left, down, right, ... covers the 2 x 4 region
        assert_eq!(deepest, 8);
        assert_eq!(f.fill_from(2, 0), 0);
    }

    #[test]
    fn claims_choose_the_largest_weighted_label() {
        let mut g = Globals::new(UpstreamDefects::Fix);
        let mut f = field(6, 6);
        // a square outline around (2, 2) of label 3
        let mut pts = Vec::new();
        for x in 1..=3 {
            for y in 1..=3 {
                if x != 2 || y != 2 {
                    pts.extend_from_slice(&[x, y]);
                }
            }
        }
        assert_eq!(
            f.claim_points(&mut g, &mut Vec::new(), 3, &pts, 2, 2, 1),
            NO_ERROR
        );
        let scan = f.assign_labels(false);
        assert_eq!(scan.claimed_5307, 0);
        // the interior (2, 2) is label 3; an outline corner has more unlabelled neighbours
        // (2.25 against 1.5), an outline side ties (2 and 2: the smaller label), the far corner
        assert_eq!(f.fill[f.at(2, 2)], 3);
        assert_eq!(f.fill[f.at(1, 1)], 0);
        assert_eq!(f.fill[f.at(2, 1)], 0);
        assert_eq!(f.fill[f.at(5, 5)], 0);
    }

    #[test]
    fn entry_5307_adds_up_under_reproduce() {
        let mut g = Globals::new(UpstreamDefects::Fix);
        let mut f = field(8, 8);
        let pts: Vec<i16> = vec![0, 1, 1, 0, 1, 1];
        assert_eq!(
            f.claim_points(&mut g, &mut Vec::new(), 5307, &pts, 0, 0, 1),
            NO_ERROR
        );
        let mut f2 = field(8, 8);
        f2.claim = f.claim.clone();
        let fresh = f.assign_labels(false);
        let kept = f2.assign_labels(true);
        assert_eq!(fresh.claimed_5307, kept.claimed_5307);
        assert!(kept.differ_5307 > 0);
        // under reproduce every place scanned after the region takes 5307
        assert_eq!(f2.fill[f2.at(7, 7)], 5307);
        assert_eq!(f.fill[f.at(7, 7)], 0);
    }
}
