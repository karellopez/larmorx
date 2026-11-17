// SPDX-License-Identifier: Apache-2.0
//! The volume-geometry text block of linear transform and surface files (spec §2, §3):
//! `writeVolGeom` and `readVolGeom`, with FS-285 to FS-288.

use larmorx_core::defects::{FS_285, FS_286, FS_287, FS_288};

use crate::clib::printf::sprintf_f64;
use crate::maths::mfiles::{CStream, Call};
use crate::mri::VolGeom;
use crate::mri::lta::init_vol_geom;
use crate::process::Process;
use crate::sse;

/// The buffer `readVolGeom` reads each line into (`fgets`: 1,039 bytes at most).
const LINE_BUFFER: usize = 1040;

/// The eight keys of the block, in the order they are written.
const KEYS: [&[u8]; 8] = [
    b"valid",
    b"filename",
    b"volume",
    b"voxelsize",
    b"xras",
    b"yras",
    b"zras",
    b"cras",
];

/// FreeSurfer's buffers for a line's first two words (FS-285, FS-286).
const KEY_BUFFER: usize = 64;
const EQ_BUFFER: usize = 2;

/// What the first word buffer holds between lines (FS-287, FS-288).
enum Kept {
    /// Never written (before the first line with a word).
    Never,
    /// The key the last line with a word left.
    Key(Vec<u8>),
}

impl Process {
    /// `writeVolGeom(fp, vg)` (§2): the eight lines of the block, as the stream receives them.
    pub fn write_vol_geom(&mut self, vg: &VolGeom) -> Vec<u8> {
        let mut b = Vec::with_capacity(400);
        let state = if vg.valid == 0 { "invalid" } else { "valid" };
        b.extend_from_slice(format!("valid = {}  # volume info {state}\n", vg.valid).as_bytes());
        b.extend_from_slice(b"filename = ");
        let end = vg
            .fname
            .iter()
            .position(|&c| c == 0)
            .unwrap_or(vg.fname.len());
        b.extend_from_slice(&vg.fname[..end]);
        b.push(b'\n');
        b.extend_from_slice(
            format!("volume = {} {} {}\n", vg.width, vg.height, vg.depth).as_bytes(),
        );
        let rows: [(&[u8], [f32; 3]); 5] = [
            (b"voxelsize = ", [vg.xsize, vg.ysize, vg.zsize]),
            (b"xras   = ", vg.x),
            (b"yras   = ", vg.y),
            (b"zras   = ", vg.z),
            (b"cras   = ", vg.c),
        ];
        for (name, v) in rows {
            b.extend_from_slice(name);
            for (k, x) in v.iter().enumerate() {
                if k > 0 {
                    b.push(b' ');
                }
                b.extend_from_slice(&sprintf_f64(b"%.15e", sse::widen(*x)));
            }
            b.push(b'\n');
        }
        b
    }

    /// `readVolGeom(fp, vg)` (§3): the block's lines from the stream into the geometry; the
    /// stream left where FreeSurfer leaves it, `errno` as the C library's `sscanf` calls leave it.
    pub fn read_vol_geom(&mut self, s: &mut CStream, vg: &mut VolGeom) {
        let reproduce = self.mode().reproduce();
        let mut counted = 0;
        let mut valid_read = false;
        // the position after the last used line (0: none, nothing done)
        let mut last_pos = 0usize;
        let mut kept = Kept::Never;
        let mut back = false;
        while let Some(piece) = s.fgets(LINE_BUFFER) {
            if piece.first() == Some(&0) || counted >= 8 {
                back = true;
                break;
            }
            let end = piece.iter().position(|&c| c == 0).unwrap_or(piece.len());
            let text = &piece[..end];
            // the first `sscanf(line, "%s %s")`
            let mut line = CStream::new(text.to_vec());
            let mut call = Call::new();
            let first = call.s(self, &mut line, None);
            let second = if first.is_some() {
                call.s(self, &mut line, None)
            } else {
                None
            };
            let key: Option<Vec<u8>> = match &first {
                Some(w) => {
                    if w.len() >= KEY_BUFFER {
                        self.note(
                            &FS_285,
                            format!(
                                "a block line's first word of {} bytes read whole (FreeSurfer's buffer holds 63)",
                                w.len()
                            ),
                        );
                    }
                    let mut k = w.clone();
                    if let Some(eq) = &second
                        && eq.len() >= EQ_BUFFER
                    {
                        if reproduce {
                            self.note(
                                &FS_286,
                                "a block line's key taken from its second word's third byte on (FreeSurfer's overflow)",
                            );
                            k = eq[EQ_BUFFER..].to_vec();
                        } else {
                            self.note(
                                &FS_286,
                                "a block line with a second word of 2 bytes or more compared by its first word",
                            );
                        }
                    }
                    kept = Kept::Key(k.clone());
                    Some(k)
                }
                None => match &kept {
                    Kept::Never => {
                        self.note(
                            &FS_288,
                            "a blank block line before any line with a word counted as no key",
                        );
                        None
                    }
                    Kept::Key(k) => {
                        if KEYS.contains(&k.as_slice()) {
                            if reproduce {
                                self.note(
                                    &FS_287,
                                    "a blank block line counted as a repeat of the line before it",
                                );
                                Some(k.clone())
                            } else {
                                self.note(
                                    &FS_287,
                                    "a blank block line used but neither counted nor read",
                                );
                                None
                            }
                        } else {
                            None
                        }
                    }
                },
            };
            if let Some(k) = key
                && let Some(i) = KEYS.iter().position(|&x| x == k.as_slice())
            {
                counted += 1;
                if i == 0 {
                    valid_read = true;
                }
                self.read_block_values(text, i, vg);
            }
            last_pos = s.pos;
        }
        if back && last_pos != 0 {
            s.seek(last_pos);
        }
        if !valid_read {
            self.put_err(b"INFO: volume info was not present.\n");
            let init = init_vol_geom();
            vg.valid = init.valid;
            vg.width = init.width;
            vg.height = init.height;
            vg.depth = init.depth;
            vg.xsize = init.xsize;
            vg.ysize = init.ysize;
            vg.zsize = init.zsize;
            vg.x = init.x;
            vg.y = init.y;
            vg.z = init.z;
            vg.c = init.c;
            vg.fname = init.fname;
        }
    }

    /// The second `sscanf` of a counted line (§3, "Values"): `%s %s`, then key `i`'s numbers or
    /// name, then the format's final white space.
    fn read_block_values(&mut self, text: &[u8], i: usize, vg: &mut VolGeom) {
        let mut line = CStream::new(text.to_vec());
        let mut call = Call::new();
        if call.s(self, &mut line, None).is_none() || call.s(self, &mut line, None).is_none() {
            return;
        }
        match i {
            0 => {
                if let Some(v) = call.d(self, &mut line) {
                    vg.valid = v;
                    call.ws(self, &mut line);
                }
            }
            1 => {
                if let Some(w) = call.s(self, &mut line, Some(1023)) {
                    vg.fname = w;
                    call.ws(self, &mut line);
                }
            }
            2 => {
                let fields = [&mut vg.width, &mut vg.height, &mut vg.depth];
                let mut all = true;
                for f in fields {
                    match call.d(self, &mut line) {
                        Some(v) => *f = v,
                        None => {
                            all = false;
                            break;
                        }
                    }
                }
                if all {
                    call.ws(self, &mut line);
                }
            }
            _ => {
                let fields: [&mut f32; 3] = match i {
                    3 => [&mut vg.xsize, &mut vg.ysize, &mut vg.zsize],
                    4 => {
                        let [a, b, c] = &mut vg.x;
                        [a, b, c]
                    }
                    5 => {
                        let [a, b, c] = &mut vg.y;
                        [a, b, c]
                    }
                    6 => {
                        let [a, b, c] = &mut vg.z;
                        [a, b, c]
                    }
                    _ => {
                        let [a, b, c] = &mut vg.c;
                        [a, b, c]
                    }
                };
                let mut all = true;
                for f in fields {
                    match call.f(self, &mut line) {
                        Some(v) => *f = v,
                        None => {
                            all = false;
                            break;
                        }
                    }
                }
                if all {
                    call.ws(self, &mut line);
                }
            }
        }
    }
}
