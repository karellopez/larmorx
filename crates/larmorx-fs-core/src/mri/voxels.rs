// SPDX-License-Identifier: Apache-2.0
//! Voxel access (spec §5): reading and writing a voxel, the derivatives, the defects of indices
//! outside the volume (FS-52, FS-53), long volumes (FS-54) and volumes without voxels (FS-58).

use larmorx_core::defects::{FS_52, FS_53, FS_54, FS_58};

use super::types;
use super::volume::Mri;
use crate::process::{Process, Stop, codes};
use crate::sse;

/// Where a voxel's element lies.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Place {
    /// Inside the volume, at this byte offset.
    In(u64),
    /// Outside the volume but inside the buffer: another voxel, at this byte offset (FS-52).
    Other(u64),
    /// Outside the buffer, or found through memory outside the volume's tables (FS-53).
    Beyond,
}

impl Mri {
    /// Whether (c, r, s, f) lies inside the volume.
    pub fn inside(&self, c: i32, r: i32, s: i32, f: i32) -> bool {
        (0..self.geom.width).contains(&c)
            && (0..self.geom.height).contains(&r)
            && (0..self.geom.depth).contains(&s)
            && (0..self.nframes).contains(&f)
    }

    /// The byte offset `e` as a place: inside the buffer or beyond it.
    fn place_of(&self, offset: Option<u64>, inside: bool) -> Place {
        let size = self.bytes_per_vox;
        let len = self.voxels.as_ref().map_or(0, |v| v.len() as u64);
        match offset {
            Some(o) if o < len && o.checked_add(size).is_some_and(|e| e <= len) => {
                if inside {
                    Place::In(o)
                } else {
                    Place::Other(o)
                }
            }
            _ => Place::Beyond,
        }
    }

    /// The place of element `c + r W + s W H + f W H D` (computed as `size_t`).
    pub fn flat_place(&self, c: i32, r: i32, s: i32, f: i32) -> Place {
        let z = |v: i32| v as i64 as u64;
        let (w, h, d) = (z(self.geom.width), z(self.geom.height), z(self.geom.depth));
        let wh = w.wrapping_mul(h);
        let e = z(c)
            .wrapping_add(z(r).wrapping_mul(w))
            .wrapping_add(z(s).wrapping_mul(wh))
            .wrapping_add(z(f).wrapping_mul(wh.wrapping_mul(d)));
        let o = e.wrapping_mul(self.bytes_per_vox);
        self.place_of(Some(o), self.inside(c, r, s, f))
    }

    /// The place found through the row tables (writes and complex volumes): slice `s + f D`'s
    /// row `r`, then column `c` of that row; a slice or row outside the tables is FS-53.
    pub fn table_place(&self, c: i32, r: i32, s: i32, f: i32) -> Place {
        let (w, h, d) = (
            i64::from(self.geom.width),
            i64::from(self.geom.height),
            i64::from(self.geom.depth),
        );
        let t = i64::from(s) + i64::from(f) * d;
        if t < 0 || t >= d * i64::from(self.nframes) || r < 0 || i64::from(r) >= h {
            return Place::Beyond;
        }
        let e = (t * h + i64::from(r)) * w + i64::from(c);
        let o = (e >= 0).then(|| (e as u64).wrapping_mul(self.bytes_per_vox));
        self.place_of(o, self.inside(c, r, s, f))
    }

    fn bytes(&self, o: u64, n: usize) -> &[u8] {
        let v = self.voxels.as_deref().expect("voxels");
        &v[o as usize..o as usize + n]
    }

    fn put(&mut self, o: u64, b: &[u8]) {
        let v = self.voxels.as_deref_mut().expect("voxels");
        v[o as usize..o as usize + b.len()].copy_from_slice(b);
    }

    /// The value of the element at `o` as a float (§5), for the types that have one.
    fn value_at(&self, o: u64) -> f32 {
        match self.kind {
            types::UCHAR => f32::from(self.bytes(o, 1)[0]),
            types::SHORT => f32::from(i16::from_le_bytes(self.bytes(o, 2).try_into().expect("2"))),
            types::USHRT => f32::from(u16::from_le_bytes(self.bytes(o, 2).try_into().expect("2"))),
            types::INT | types::RGB => {
                i32::from_le_bytes(self.bytes(o, 4).try_into().expect("4")) as f32
            }
            types::LONG => i64::from_le_bytes(self.bytes(o, 8).try_into().expect("8")) as f32,
            types::FLOAT => f32::from_le_bytes(self.bytes(o, 4).try_into().expect("4")),
            _ => -1e10,
        }
    }
}

impl Process {
    /// FS-58 (both modes): a voxel of a volume without voxels.
    pub(crate) fn no_voxels(&mut self, function: &str) -> Result<(), Stop> {
        self.note(
            &FS_58,
            format!("{function}() on a volume without voxels: refused (FreeSurfer crashes)"),
        );
        self.error_exit(
            codes::BAD_PARM,
            format!("{function}(): the volume has no voxels (FreeSurfer crashes here)").as_bytes(),
        )
    }

    /// The read of a place outside the volume (FS-52, FS-53): `Ok(offset)` to read, `Err(value)`
    /// for the value given instead.
    fn outside_read(&mut self, m: &Mri, place: Place, at: [i32; 4]) -> Result<u64, f32> {
        let outside = sse::narrow(m.outside_val);
        match place {
            Place::In(o) => Ok(o),
            Place::Other(o) => {
                if self.mode().reproduce() {
                    self.note(
                        &FS_52,
                        format!("voxel {at:?} outside the volume read as another voxel"),
                    );
                    Ok(o)
                } else {
                    self.note(
                        &FS_52,
                        format!("voxel {at:?} outside the volume read as outside_val"),
                    );
                    Err(outside)
                }
            }
            Place::Beyond => {
                if self.mode().reproduce() {
                    self.note(
                        &FS_53,
                        format!("voxel {at:?} outside the volume's buffer read as 0 (FreeSurfer reads memory contents)"),
                    );
                    Err(0.0)
                } else {
                    self.note(
                        &FS_53,
                        format!("voxel {at:?} outside the volume's buffer read as outside_val"),
                    );
                    Err(outside)
                }
            }
        }
    }

    /// The write of a place outside the volume (FS-52, FS-53): the offset to write, or none.
    fn outside_write(&mut self, place: Place, at: [i32; 4]) -> Option<u64> {
        match place {
            Place::In(o) => Some(o),
            Place::Other(o) => {
                if self.mode().reproduce() {
                    self.note(
                        &FS_52,
                        format!("voxel {at:?} outside the volume written into another voxel"),
                    );
                    Some(o)
                } else {
                    self.note(
                        &FS_52,
                        format!("voxel {at:?} outside the volume not written"),
                    );
                    None
                }
            }
            Place::Beyond => {
                self.note(
                    &FS_53,
                    format!("voxel {at:?} outside the volume's buffer not written (FreeSurfer writes outside the volume)"),
                );
                None
            }
        }
    }

    /// `MRIgetVoxVal(mri, c, r, s, f, mode)` (§5).
    pub fn mri_get_vox_val(
        &mut self,
        m: &Mri,
        c: i32,
        r: i32,
        s: i32,
        f: i32,
        mode: i32,
    ) -> Result<f32, Stop> {
        self.get_vox(m, c, r, s, f, mode, "MRIgetVoxVal")
    }

    #[allow(clippy::too_many_arguments)]
    fn get_vox(
        &mut self,
        m: &Mri,
        c: i32,
        r: i32,
        s: i32,
        f: i32,
        mode: i32,
        function: &str,
    ) -> Result<f32, Stop> {
        if !m.has_voxels() {
            self.no_voxels(function)?;
            return Ok(0.0);
        }
        if c < 0 || r < 0 || s < 0 {
            return Ok(sse::narrow(m.outside_val));
        }
        let at = [c, r, s, f];
        match m.kind {
            types::FLOAT_COMPLEX => {
                if mode != 0 && mode != 1 {
                    self.error_exit(
                        codes::UNSUPPORTED,
                        format!("MRIgetVoxVal(): unsupported operation {mode}").as_bytes(),
                    )?;
                    return Ok(0.0);
                }
                let place = m.table_place(c, r, s, f);
                Ok(match self.outside_read(m, place, at) {
                    Ok(o) => {
                        let k = o + 4 * mode as u64;
                        f32::from_le_bytes(m.bytes(k, 4).try_into().expect("4"))
                    }
                    Err(v) => v,
                })
            }
            types::UCHAR
            | types::SHORT
            | types::USHRT
            | types::INT
            | types::RGB
            | types::LONG
            | types::FLOAT => {
                let place = m.flat_place(c, r, s, f);
                Ok(match self.outside_read(m, place, at) {
                    Ok(o) => m.value_at(o),
                    Err(v) => v,
                })
            }
            _ => Ok(-1e10),
        }
    }

    /// `MRIsetVoxVal(mri, c, r, s, f, value, mode)` (§5): 0, or 1 (nothing stored) for the types
    /// without a stored value.
    #[allow(clippy::too_many_arguments)]
    pub fn mri_set_vox_val(
        &mut self,
        m: &mut Mri,
        c: i32,
        r: i32,
        s: i32,
        f: i32,
        v: f32,
        mode: i32,
    ) -> Result<i32, Stop> {
        if !m.has_voxels() {
            self.no_voxels("MRIsetVoxVal")?;
            return Ok(0);
        }
        let at = [c, r, s, f];
        let bytes: Vec<u8> = match m.kind {
            types::FLOAT_COMPLEX => {
                if mode != 0 && mode != 1 {
                    self.error_exit(
                        codes::UNSUPPORTED,
                        format!("MRIsetVoxVal(): unsupported operation {mode}").as_bytes(),
                    )?;
                    return Ok(0);
                }
                let place = m.table_place(c, r, s, f);
                if let Some(o) = self.outside_write(place, at) {
                    m.put(o + 4 * mode as u64, &v.to_le_bytes());
                }
                return Ok(0);
            }
            types::UCHAR => {
                let x = v.clamp_c(0.0, 255.0);
                let i = self.nint(sse::widen(x));
                vec![i as u8]
            }
            types::SHORT => {
                let x = v.clamp_c(-32768.0, 32767.0);
                let i = self.nint(sse::widen(x));
                (i as i16).to_le_bytes().to_vec()
            }
            types::USHRT => {
                let x = v.clamp_c(0.0, 65535.0);
                let i = self.nint(sse::widen(x));
                (i as u16).to_le_bytes().to_vec()
            }
            types::INT | types::RGB => {
                let x = v.clamp_c(-2147483648.0, 2147483648.0);
                let i = self.nint(sse::widen(x));
                i.to_le_bytes().to_vec()
            }
            types::FLOAT => v.to_le_bytes().to_vec(),
            types::LONG => {
                let place = m.table_place(c, r, s, f);
                let Some(o) = self.outside_write(place, at) else {
                    return Ok(0);
                };
                self.set_long(m, o, c, v);
                return Ok(0);
            }
            _ => return Ok(1),
        };
        let place = m.table_place(c, r, s, f);
        if let Some(o) = self.outside_write(place, at) {
            m.put(o, &bytes);
        }
        Ok(0)
    }

    /// A long voxel's write (FS-54) at byte `o`, column `c`.
    fn set_long(&mut self, m: &mut Mri, o: u64, c: i32, v: f32) {
        let x = v.clamp_c(-9.223_372e18, 9.223_372e18);
        let xd = sse::widen(x);
        let fixed = if xd < 0.0 { xd - 0.5 } else { xd + 0.5 } as i64;
        // FreeSurfer's: the int rounding, stored as a long, then as 4 bytes into the row's voxel
        // c / 2 (low half for even c, high for odd).
        let row = o - (c as i64 as u64).wrapping_mul(8);
        let half = row.wrapping_add((c as i64 as u64).wrapping_mul(4));
        let before = m.voxels.clone().expect("voxels");
        let reproduce = self.mode().reproduce();
        let i = if reproduce {
            self.nint(xd)
        } else {
            let mut q = Process::silent(self.env.clone(), self.mode());
            q.nint(xd)
        };
        let mut theirs = before.clone();
        theirs[o as usize..o as usize + 8].copy_from_slice(&i64::from(i).to_le_bytes());
        if half
            .checked_add(4)
            .is_some_and(|e| e <= theirs.len() as u64)
        {
            theirs[half as usize..half as usize + 4].copy_from_slice(&i.to_le_bytes());
        }
        let mut ours = before;
        ours[o as usize..o as usize + 8].copy_from_slice(&fixed.to_le_bytes());
        if theirs != ours {
            if reproduce {
                self.note(
                    &FS_54,
                    "a long voxel stored as FreeSurfer stores it: rounded to an int, also into half of another voxel",
                );
            } else {
                self.note(
                    &FS_54,
                    "a long voxel stored once, rounded in the long range",
                );
            }
        }
        m.voxels = Some(if reproduce { theirs } else { ours });
    }

    /// `MRIgetVoxDx`, `Dy`, `Dz` (§5): `(V(+1) - V(-1)) / (2.0 size)` along axis `axis` (0, 1, 2).
    pub fn mri_get_vox_d(
        &mut self,
        m: &Mri,
        axis: usize,
        c: i32,
        r: i32,
        s: i32,
        f: i32,
    ) -> Result<f32, Stop> {
        let name = ["MRIgetVoxDx", "MRIgetVoxDy", "MRIgetVoxDz"][axis];
        let step = |k: i32| match axis {
            0 => (c + k, r, s),
            1 => (c, r + k, s),
            _ => (c, r, s + k),
        };
        let (c1, r1, s1) = step(1);
        let (c0, r0, s0) = step(-1);
        let a = self.get_vox(m, c1, r1, s1, f, 0, name)?;
        let b = self.get_vox(m, c0, r0, s0, f, 0, name)?;
        let size = m.geom.size(axis);
        let d = sse::div64(
            sse::widen(sse::sub32(a, b)),
            sse::mul64(2.0, sse::widen(size)),
        );
        Ok(sse::narrow(d))
    }
}

/// C's clipping `if (x < lo) x = lo; if (x > hi) x = hi;` (a NaN stays).
trait ClampC {
    fn clamp_c(self, lo: f32, hi: f32) -> f32;
}

impl ClampC for f32 {
    fn clamp_c(self, lo: f32, hi: f32) -> f32 {
        let mut x = self;
        if x < lo {
            x = lo;
        }
        if x > hi {
            x = hi;
        }
        x
    }
}
