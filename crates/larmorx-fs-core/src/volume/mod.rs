// SPDX-License-Identifier: Apache-2.0
//! FreeSurfer 8.2.0's volume functions (`fs.volume`), a clean room of
//! `specs/freesurfer-volume.md` parts 1 to 7a: the volume-geometry text block (§2, §3), the
//! geometry's copies, matrices, comparison and printer (§4), the voxel arithmetic (§6-§11),
//! coordinates and geometry (§13-§16), the samplers (§18), the moves (§19), the head's box (§20),
//! the reader of linear transforms (§21), the CMA labels and segmentation functions (§23),
//! resolution, planes, fills, resampling by whole factors and changes of type (§25), the
//! frame-range samplers, the trilinear kernel and splatting (§27, `frames`), derivatives,
//! gradients and neighbour counts (§28, `deriv`), linear transforms of a volume (§29, `linear`)
//! and resampling onto a template's grid and replacing a value (§31, `regrid`); statistics,
//! orientation strings and voxel-to-RAS matrices (§33, `info`); slices as 2-D images (§34,
//! `image`), neighbourhoods and labels (§35, `nbhd`); and of §54's parts ready for the clean
//! room, the inverses' fill and the change from physical-voxel to RAS-to-RAS (`ltaops`).
//!
//! Functions that take a destination (§6) take a [`Dst`]: none (the function makes one), the
//! first source itself, the second source itself, or the caller's volume; they return a
//! [`VolOut`] saying which volume is the result. Every function that prints, ends the program
//! or meets a defect is a method of the program's [`Process`](crate::Process).

pub mod arith;
pub mod cma;
pub mod deriv;
pub mod extract;
pub mod frames;
pub mod geomblock;
pub mod geometry;
pub mod image;
pub mod info;
pub mod linear;
pub mod ltaops;
pub mod ltaread;
pub mod moves;
pub mod nbhd;
pub mod regrid;
pub mod resample;
pub mod sample;
pub mod skull;
pub mod stats;
pub mod vgcopy;

use larmorx_core::defects::{FS_289, FS_290, FS_960};

use crate::mri::Mri;
use crate::mri::voxels::Place;
use crate::process::{Process, Stop};

/// A function's destination (§6).
pub enum Dst<'a> {
    /// None: the function makes one.
    New,
    /// The first source itself.
    Src,
    /// The second source itself (functions of two volumes).
    Other,
    /// The caller's volume.
    Given(&'a mut Mri),
}

/// Which volume a function returned (§6).
#[derive(Debug)]
#[allow(clippy::large_enum_variant)]
pub enum VolOut {
    /// None (`NULL`).
    Nothing,
    /// A volume the function made.
    New(Mri),
    /// The first source.
    Src,
    /// The second source.
    Other,
    /// The caller's destination.
    Given,
}

/// Where the destination is while a function works.
#[allow(clippy::large_enum_variant)]
enum Slot<'a> {
    None,
    Own(Mri),
    A,
    B,
    Given(&'a mut Mri),
}

/// The volumes of one call: the first source, the second (when there is one) and the
/// destination, which may be either source.
pub(crate) struct Vols<'a> {
    a: &'a mut Mri,
    b: Option<&'a mut Mri>,
    d: Slot<'a>,
}

impl<'a> Vols<'a> {
    pub(crate) fn new(a: &'a mut Mri, b: Option<&'a mut Mri>, dst: Dst<'a>) -> Vols<'a> {
        let d = match dst {
            Dst::New => Slot::None,
            Dst::Src => Slot::A,
            Dst::Other => {
                if b.is_some() {
                    Slot::B
                } else {
                    Slot::None
                }
            }
            Dst::Given(m) => Slot::Given(m),
        };
        Vols { a, b, d }
    }

    pub(crate) fn a(&self) -> &Mri {
        self.a
    }

    pub(crate) fn b(&self) -> &Mri {
        self.b.as_deref().expect("a second volume")
    }

    /// Whether the destination is the first source itself.
    pub(crate) fn d_is_a(&self) -> bool {
        matches!(self.d, Slot::A)
    }

    pub(crate) fn has_dst(&self) -> bool {
        !matches!(self.d, Slot::None)
    }

    pub(crate) fn set_dst(&mut self, m: Mri) {
        self.d = Slot::Own(m);
    }

    pub(crate) fn d(&self) -> &Mri {
        match &self.d {
            Slot::None => panic!("no destination"),
            Slot::Own(m) => m,
            Slot::A => self.a,
            Slot::B => self.b(),
            Slot::Given(m) => m,
        }
    }

    pub(crate) fn d_mut(&mut self) -> &mut Mri {
        match &mut self.d {
            Slot::None => panic!("no destination"),
            Slot::Own(m) => m,
            Slot::A => self.a,
            Slot::B => self.b.as_deref_mut().expect("a second volume"),
            Slot::Given(m) => m,
        }
    }

    /// The volume `w` mutably (0 the first source, 1 the second, 2 the destination).
    pub(crate) fn vol_mut(&mut self, w: usize) -> &mut Mri {
        match w {
            0 => self.a,
            1 => self.b.as_deref_mut().expect("a second volume"),
            _ => self.d_mut(),
        }
    }

    /// The result: the destination, as which volume it is.
    pub(crate) fn out(self) -> VolOut {
        match self.d {
            Slot::None => VolOut::Nothing,
            Slot::Own(m) => VolOut::New(m),
            Slot::A => VolOut::Src,
            Slot::B => VolOut::Other,
            Slot::Given(_) => VolOut::Given,
        }
    }
}

/// The byte offset of element `x` of row `y` of slice `z + f depth` through the volume's row
/// tables, with elements of `esize` bytes (direct access, §6): inside the volume, at another
/// place of its buffer (FS-289) or outside it (FS-290). `tensor_first`: FS-960's FreeSurfer
/// rows of a tensor volume (every row at its first voxel), else each row at its own place.
pub fn direct_place(
    m: &Mri,
    x: i64,
    y: i64,
    z: i64,
    f: i64,
    esize: u64,
    tensor_first: bool,
) -> Place {
    row_byte_place(m, x * esize as i64, y, z, f, esize, tensor_first)
}

/// The place of `n` bytes from byte `byte` of row `y` of slice `z + f depth` (§6): inside the
/// volume when they lie within that row of a voxel the volume has. With `tensor_first`, every row
/// of a tensor volume (type 6) starts at the volume's first voxel (FS-960: FreeSurfer's row
/// tables of a tensor volume); else row `y` of slice `s` starts at byte `(s height + y) w b`.
pub fn row_byte_place(
    m: &Mri,
    byte: i64,
    y: i64,
    z: i64,
    f: i64,
    n: u64,
    tensor_first: bool,
) -> Place {
    let g = &m.geom;
    let (w, h, d) = (i64::from(g.width), i64::from(g.height), i64::from(g.depth));
    let t = z + f * d;
    if t < 0 || t >= d * i64::from(m.nframes) || y < 0 || y >= h {
        return Place::Beyond;
    }
    let row_len = w * m.bytes_per_vox as i64;
    let row = if tensor_first && m.kind == crate::mri::types::TENSOR {
        0
    } else {
        (t * h + y) * row_len
    };
    let o = row + byte;
    let len = m.voxels.as_ref().map_or(0, |v| v.len() as i64);
    if o < 0 || o + n as i64 > len {
        return Place::Beyond;
    }
    let inside = byte >= 0
        && byte + n as i64 <= row_len
        && (0..d).contains(&z)
        && (0..i64::from(m.nframes)).contains(&f);
    if inside {
        Place::In(o as u64)
    } else {
        Place::Other(o as u64)
    }
}

impl Process {
    /// [`direct_place`] in this program's mode (FS-960, noted where a direct walk reaches a row of
    /// a tensor volume other than slice 0's row 0).
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn place_of(
        &mut self,
        m: &Mri,
        x: i64,
        y: i64,
        z: i64,
        f: i64,
        esize: u64,
    ) -> Place {
        self.row_place(m, x * esize as i64, y, z, f, esize)
    }

    /// [`row_byte_place`] in this program's mode (FS-960, noted as [`Process::place_of`]).
    pub(crate) fn row_place(
        &mut self,
        m: &Mri,
        byte: i64,
        y: i64,
        z: i64,
        f: i64,
        n: u64,
    ) -> Place {
        let reproduce = self.mode().reproduce();
        if m.kind == crate::mri::types::TENSOR {
            let t = z + f * i64::from(m.geom.depth);
            if (t != 0 || y != 0) && !self.noted(&FS_960) {
                let how = if reproduce {
                    "at the volume's first voxel, as FreeSurfer's"
                } else {
                    "at its own place (FreeSurfer finds every row at the first voxel)"
                };
                self.note(
                    &FS_960,
                    format!("a tensor volume walked directly: each row {how}"),
                );
            }
        }
        row_byte_place(m, byte, y, z, f, n, reproduce)
    }

    /// A direct read of `n` bytes at a place (§6): inside, the bytes; at another voxel, the bytes
    /// with `reproduce` and zeros by default (FS-289); outside the buffer, zeros (FS-290).
    pub(crate) fn direct_read(&mut self, m: &Mri, place: Place, n: usize, what: &str) -> Vec<u8> {
        let v = m.voxels.as_deref().unwrap_or(&[]);
        match place {
            Place::In(o) => v[o as usize..o as usize + n].to_vec(),
            Place::Other(o) => {
                if self.mode().reproduce() {
                    self.note(
                        &FS_289,
                        format!("{what}: another voxel of the volume's buffer read"),
                    );
                    v[o as usize..o as usize + n].to_vec()
                } else {
                    self.note(
                        &FS_289,
                        format!("{what}: a voxel outside the volume read as 0"),
                    );
                    vec![0; n]
                }
            }
            Place::Beyond => {
                self.note(
                    &FS_290,
                    format!("{what}: memory outside the volume read as 0 (FreeSurfer reads memory it does not own)"),
                );
                vec![0; n]
            }
        }
    }

    /// [`Process::direct_read`] of at most 8 bytes into a fixed array (no allocation: the
    /// samplers' voxel reads).
    pub(crate) fn direct_read8(&mut self, m: &Mri, place: Place, n: usize, what: &str) -> [u8; 8] {
        let mut out = [0u8; 8];
        if let (Place::In(o), Some(v)) = (place, m.voxels.as_deref()) {
            out[..n].copy_from_slice(&v[o as usize..o as usize + n]);
            return out;
        }
        let b = self.direct_read(m, place, n, what);
        out[..n].copy_from_slice(&b);
        out
    }

    /// A direct write of bytes at a place (§6): inside, written; at another voxel, written with
    /// `reproduce` and dropped by default (FS-289); outside the buffer, dropped (FS-290).
    pub(crate) fn direct_write(&mut self, m: &mut Mri, place: Place, b: &[u8], what: &str) {
        let o = match place {
            Place::In(o) => o,
            Place::Other(o) => {
                if self.mode().reproduce() {
                    self.note(
                        &FS_289,
                        format!("{what}: another voxel of the volume's buffer written"),
                    );
                    o
                } else {
                    self.note(
                        &FS_289,
                        format!("{what}: a voxel outside the volume not written"),
                    );
                    return;
                }
            }
            Place::Beyond => {
                self.note(
                    &FS_290,
                    format!("{what}: memory outside the volume not written (FreeSurfer writes memory it does not own)"),
                );
                return;
            }
        };
        if let Some(v) = m.voxels.as_deref_mut() {
            v[o as usize..o as usize + b.len()].copy_from_slice(b);
        }
    }

    /// FS-58 for a volume walked directly without voxels: the end of the program.
    pub(crate) fn direct_needs_voxels(&mut self, m: &Mri, function: &str) -> Result<(), Stop> {
        if !m.has_voxels() {
            self.no_voxels(function)?;
        }
        Ok(())
    }
}
