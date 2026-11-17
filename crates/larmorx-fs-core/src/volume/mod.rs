// SPDX-License-Identifier: Apache-2.0
//! FreeSurfer 8.2.0's volume functions (`fs.volume`), a clean room of
//! `specs/freesurfer-volume.md` parts 1 to 3: the volume-geometry text block (§2, §3), the
//! geometry's copies, matrices, comparison and printer (§4), the voxel arithmetic (§6-§11) and
//! coordinates and geometry (§13-§16).
//!
//! Functions that take a destination (§6) take a [`Dst`]: none (the function makes one), the
//! first source itself, the second source itself, or the caller's volume; they return a
//! [`VolOut`] saying which volume is the result. Every function that prints, ends the program
//! or meets a defect is a method of the program's [`Process`](crate::Process).

pub mod arith;
pub mod extract;
pub mod geomblock;
pub mod geometry;
pub mod sample;
pub mod stats;
pub mod vgcopy;

use larmorx_core::defects::{FS_289, FS_290};

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
/// place of its buffer (FS-289) or outside it (FS-290).
pub fn direct_place(m: &Mri, x: i64, y: i64, z: i64, f: i64, esize: u64) -> Place {
    row_byte_place(m, x * esize as i64, y, z, f, esize)
}

/// The place of `n` bytes from byte `byte` of row `y` of slice `z + f depth` (§6): inside the
/// volume when they lie within that row of a voxel the volume has. Every row of a tensor volume
/// (type 6) starts at the volume's first voxel (§6: FreeSurfer's row tables of a tensor volume,
/// met only by FS-291's and FS-301's `reproduce`).
pub fn row_byte_place(m: &Mri, byte: i64, y: i64, z: i64, f: i64, n: u64) -> Place {
    let g = &m.geom;
    let (w, h, d) = (i64::from(g.width), i64::from(g.height), i64::from(g.depth));
    let t = z + f * d;
    if t < 0 || t >= d * i64::from(m.nframes) || y < 0 || y >= h {
        return Place::Beyond;
    }
    let row_len = w * m.bytes_per_vox as i64;
    let row = if m.kind == crate::mri::types::TENSOR {
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
