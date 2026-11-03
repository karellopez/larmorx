// SPDX-License-Identifier: LicenseRef-FSL
// Translated to Rust from FSL 6.0.7.23 (fsl-basisfield 2203.4: basisfield.h, basisfield.cpp,
// splinefield.h, splinefield.cpp). FSL is Copyright (C) 2007 University of Oxford (Jesper
// Andersson). Rust translation Copyright 2026 Karel Lopez Vilaret; modified from the original.
// Distributed under the FSL Licence (see LICENSE).
//! `BASISFIELD::splinefield` with the parts of its base class `basisfield` it uses: a field on a
//! voxel grid as a sum of B-splines (quadratic or cubic) whose knots are an integer number of
//! voxels apart. The field and its first derivatives at every voxel, at any point and outside the
//! field of view; `Jᵀe` and `JᵀJ` of the splines against an image (the Gauss-Newton parts of a
//! cost); the membrane and bending energies, their gradients and Hessians.
//!
//! Sparse results are compressed columns ([`Csc`]: FSL's `irp`, `jcp` and `valp` arrays, explicit
//! zeros included), as FSL hands them to `SparseBFMatrix<float>` or `<double>`.
//!
//! FSL computes `Jᵀe`, `JᵀJ` and the Hessians on `nthr` threads when asked, each column (or
//! element) by the same operations as on one thread: the threaded `JᵀJ` computes the elements
//! whose row index is at least the column's in each direction and copies them across the
//! diagonals, the single-threaded one copies the same elements from the columns already made, so
//! both store the same values. This translation computes them on one thread.

use super::splines::{Spline1D, Spline3D};
use crate::arma::Mat;
use crate::volume::{FslError, Volume};

/// `BASISFIELD::MAX_SIZE`: the largest field size along any direction.
pub const MAX_SIZE: u32 = 2048;

/// `FieldIndex`: the field or one of its first derivatives.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FieldIndex {
    Field = 0,
    Dfdx = 1,
    Dfdy = 2,
    Dfdz = 3,
}

impl FieldIndex {
    pub fn from_index(i: usize) -> FieldIndex {
        match i {
            0 => FieldIndex::Field,
            1 => FieldIndex::Dfdx,
            2 => FieldIndex::Dfdy,
            _ => FieldIndex::Dfdz,
        }
    }
}

/// `BasisfieldException(msg)`: `what()` is `"BasisField:: msg=" + msg`.
pub fn basisfield_error(msg: &str) -> FslError {
    FslError {
        printed: Vec::new(),
        type_name: "BASISFIELD::BasisfieldException",
        what: format!("BasisField:: msg={msg}"),
    }
}

/// A sparse matrix as compressed columns: column `c`'s row indices are
/// `irp[jcp[c]..jcp[c + 1]]` (increasing) and its values `val[...]`, explicit zeros kept.
#[derive(Clone, Debug, PartialEq)]
pub struct Csc {
    pub m: u32,
    pub n: u32,
    pub irp: Vec<u32>,
    pub jcp: Vec<u32>,
    pub val: Vec<f64>,
}

impl Csc {
    /// The value at `(row, col)` (0 where nothing is stored).
    pub fn get(&self, row: u32, col: u32) -> f64 {
        get_val(row, col, &self.irp, &self.jcp, &self.val)
    }
}

/// `samesize(v1, v2, 3, checkdim)` (newimage): the sizes along x, y and z, and with `checkdim`
/// the voxel sizes within 0.01 % of their mean.
pub fn samesize3<S: crate::volume::Elem, D: crate::volume::Elem>(
    v1: &Volume<S>,
    v2: &Volume<D>,
    checkdim: bool,
) -> bool {
    let mut same = v1.xsize() == v2.xsize();
    same = same && v1.ysize() == v2.ysize();
    same = same && v1.zsize() == v2.zsize();
    if checkdim {
        let close = |a: f32, b: f32| f64::from((a - b).abs()) <= 5e-5 * f64::from(a + b);
        let mut s = close(v1.xdim(), v2.xdim());
        s = s && close(v1.ydim(), v2.ydim());
        s = s && close(v1.zdim(), v2.zdim());
        same = same && s;
    }
    same
}

/// `splinefield` (with `basisfield`'s data).
#[derive(Clone, Debug)]
pub struct SplineField {
    ndim: u32,
    sz: [u32; 3],
    vxs: [f64; 3],
    coef: Vec<f64>,
    futd: [bool; 4],
    field: [Option<Vec<f64>>; 4],
    sp: Spline3D,
    dsp: [Spline3D; 3],
    nthr: u32,
}

impl SplineField {
    /// `splinefield(psz, pvxs, ksp, order, nthr)`: the coefficients all 0. `ksp` has one knot
    /// spacing per direction; FSL reads three (fewer is refused here as a dimensionality
    /// mismatch).
    pub fn new(
        psz: &[u32],
        pvxs: &[f64],
        ksp: &[u32],
        order: i32,
        nthr: u32,
    ) -> Result<SplineField, FslError> {
        // basisfield::basisfield
        if psz.is_empty() || psz.len() > 3 {
            return Err(basisfield_error(
                "basisfield::basisfield::Invalid dimensionality of field",
            ));
        }
        if psz.len() != pvxs.len() {
            return Err(basisfield_error(
                "basisfield::basisfield:: Dimensionality mismatch between psz and pvxs",
            ));
        }
        let mut sz = [1u32; 3];
        let mut vxs = [0.0f64; 3];
        for i in 0..3 {
            if i < psz.len() && (psz[i] < 1 || psz[i] > MAX_SIZE) {
                return Err(basisfield_error(
                    "basisfield::basisfield::Invalid size of field",
                ));
            }
            sz[i] = if i < psz.len() { psz[i] } else { 1 };
            vxs[i] = if i < pvxs.len() { pvxs[i] } else { 0.0 };
        }
        let ndim = psz.len() as u32;
        if ksp.len() < 3 {
            return Err(basisfield_error(
                "splinefield::splinefield: Dimensionality mismatch",
            ));
        }
        let k3 = [ksp[0], ksp[1], ksp[2]];
        // `_sp(order, ksp)`: Spline1D refuses an order other than 2 or 3 first.
        let sp = Spline3D::new(order as u32, k3, [0, 0, 0])?;
        if !(2..=3).contains(&order) {
            return Err(basisfield_error(
                "splinefield::splinefield: Only quadratic and cubic splines implemented yet",
            ));
        }
        if ksp.len() != ndim as usize {
            return Err(basisfield_error(
                "splinefield::splinefield: Dimensionality mismatch",
            ));
        }
        let dsp = [
            Spline3D::new(order as u32, k3, [1, 0, 0])?,
            Spline3D::new(order as u32, k3, [0, 1, 0])?,
            Spline3D::new(order as u32, k3, [0, 0, 1])?,
        ];
        let mut f = SplineField {
            ndim,
            sz,
            vxs,
            coef: Vec::new(),
            futd: [false; 4],
            field: [None, None, None, None],
            sp,
            dsp,
            nthr,
        };
        f.coef = vec![0.0; f.coef_sz() as usize];
        Ok(f)
    }

    pub fn ndim(&self) -> u32 {
        self.ndim
    }
    pub fn field_sz_x(&self) -> u32 {
        self.sz[0]
    }
    pub fn field_sz_y(&self) -> u32 {
        self.sz[1]
    }
    pub fn field_sz_z(&self) -> u32 {
        self.sz[2]
    }
    pub fn field_sz(&self) -> u32 {
        self.sz[0] * self.sz[1] * self.sz[2]
    }
    pub fn field_size(&self) -> [u32; 3] {
        self.sz
    }
    pub fn vxs_x(&self) -> f64 {
        self.vxs[0]
    }
    pub fn vxs_y(&self) -> f64 {
        self.vxs[1]
    }
    pub fn vxs_z(&self) -> f64 {
        self.vxs[2]
    }
    pub fn voxel_size(&self) -> [f64; 3] {
        self.vxs
    }
    pub fn coef_sz_x(&self) -> u32 {
        self.sp.ncoef(0, self.sz[0])
    }
    pub fn coef_sz_y(&self) -> u32 {
        if self.ndim < 2 {
            1
        } else {
            self.sp.ncoef(1, self.sz[1])
        }
    }
    pub fn coef_sz_z(&self) -> u32 {
        if self.ndim < 3 {
            1
        } else {
            self.sp.ncoef(2, self.sz[2])
        }
    }
    pub fn coef_size(&self) -> [u32; 3] {
        [self.coef_sz_x(), self.coef_sz_y(), self.coef_sz_z()]
    }
    pub fn coef_sz(&self) -> u32 {
        self.coef_sz_x() * self.coef_sz_y() * self.coef_sz_z()
    }
    pub fn order(&self) -> u32 {
        self.sp.order()
    }
    pub fn ksp(&self) -> [u32; 3] {
        self.sp.knot_spacings()
    }
    pub fn kernel_sz(&self) -> [u32; 3] {
        [
            self.sp.kernel_size(0),
            self.sp.kernel_size(1),
            self.sp.kernel_size(2),
        ]
    }
    pub fn nthreads(&self) -> u32 {
        self.nthr
    }
    pub fn set_nthreads(&mut self, n: u32) {
        self.nthr = n;
    }
    pub fn up_to_date(&self, fi: FieldIndex) -> bool {
        self.futd[fi as usize]
    }

    /// The field (or a derivative) at every voxel when it is up to date ([`SplineField::update`]
    /// first): what `Get(fi)` returns then, borrowed without `&mut` (several at once).
    pub fn field_cache(&self, fi: FieldIndex) -> Option<&[f64]> {
        if self.futd[fi as usize] {
            self.field[fi as usize].as_deref()
        } else {
            None
        }
    }

    /// `GetCoef()`.
    pub fn coefs(&self) -> &[f64] {
        &self.coef
    }

    /// `GetCoef(i)`.
    pub fn get_coef(&self, i: u32) -> Result<f64, FslError> {
        if i >= self.coef_sz() {
            return Err(basisfield_error("basisfield::get_coef: i out of range"));
        }
        Ok(self.coef[i as usize])
    }

    /// `GetCoef(i, j, k)`.
    pub fn get_coef3(&self, i: u32, j: u32, k: u32) -> Result<f64, FslError> {
        let (cx, cy, cz) = (self.coef_sz_x(), self.coef_sz_y(), self.coef_sz_z());
        if i >= cx || j >= cy || k >= cz {
            return Err(basisfield_error(
                "basisfield::get_coef: i, j or k out of range",
            ));
        }
        Ok(self.coef[(k * cx * cy + j * cx + i) as usize])
    }

    /// `SetCoef(pcoef)`: the fields are out of date afterwards.
    pub fn set_coef(&mut self, pcoef: &[f64]) -> Result<(), FslError> {
        if pcoef.len() != self.coef_sz() as usize {
            return Err(basisfield_error(
                "basisfield::SetCoef::Mismatch between input vector and # of coefficients",
            ));
        }
        self.coef.copy_from_slice(pcoef);
        self.futd = [false; 4];
        Ok(())
    }

    /// `ScaleField(sf)`: `SetCoef(sf * coef)`.
    pub fn scale_field(&mut self, sf: f64) {
        let c: Vec<f64> = self.coef.iter().map(|&c| sf * c).collect();
        self.coef = c;
        self.futd = [false; 4];
    }

    fn deriv_spline(&self, fi: FieldIndex) -> Result<Spline3D, FslError> {
        let mut deriv = [0u32; 3];
        if fi != FieldIndex::Field {
            deriv[fi as usize - 1] = 1;
        }
        Spline3D::new(self.sp.order(), self.sp.knot_spacings(), deriv)
    }

    /// `Peek(double x, double y, double z, fi)`: the field (or a derivative) at a point given
    /// in voxels.
    pub fn peek(&self, x: f64, y: f64, z: f64, fi: FieldIndex) -> Result<f64, FslError> {
        let owned;
        let sp = if fi == FieldIndex::Field {
            &self.sp
        } else {
            owned = self.deriv_spline(fi)?;
            &owned
        };
        let vox = [x, y, z];
        let (first, last) = sp.range_of_splines(vox, self.coef_size());
        let mut rval = 0.0;
        for c2 in first[2]..last[2] {
            for c1 in first[1]..last[1] {
                for c0 in first[0]..last[0] {
                    rval +=
                        self.get_coef3(c0, c1, c2)? * sp.spline_value_at_voxel(vox, [c0, c1, c2]);
                }
            }
        }
        Ok(rval)
    }

    /// `basisfield::Peek(unsigned x, unsigned y, unsigned z, fi)`: the field at a voxel.
    pub fn peek_voxel(&mut self, x: u32, y: u32, z: u32, fi: FieldIndex) -> Result<f64, FslError> {
        if x >= self.sz[0] || y >= self.sz[1] || z >= self.sz[2] {
            return Err(basisfield_error(
                "basisfield::PeekField:: Co-ordinates out of bounds",
            ));
        }
        if !self.up_to_date(fi) {
            self.update(fi)?;
        }
        let f = self.field[fi as usize].as_ref().expect("updated");
        Ok(f[(z * self.sz[0] * self.sz[1] + y * self.sz[0] + x) as usize])
    }

    /// `PeekWide(i, j, k, fi)`: inside the field as [`Self::peek_voxel`], outside by summing the
    /// splines that reach the voxel (`peek_outside_fov`).
    pub fn peek_wide(&mut self, i: i32, j: i32, k: i32, fi: FieldIndex) -> Result<f64, FslError> {
        let inside = !(i < 0
            || j < 0
            || k < 0
            || i as u32 >= self.sz[0]
            || j as u32 >= self.sz[1]
            || k as u32 >= self.sz[2]);
        if inside {
            return self.peek_voxel(i as u32, j as u32, k as u32, fi);
        }
        let vox = [f64::from(i), f64::from(j), f64::from(k)];
        let csz = self.coef_size();
        let sp = if fi == FieldIndex::Field {
            &self.sp
        } else {
            &self.dsp[fi as usize - 1]
        };
        let (first, last) = sp.range_of_splines(vox, csz);
        let mut rval = 0.0;
        for c2 in first[2]..last[2] {
            for c1 in first[1]..last[1] {
                for c0 in first[0]..last[0] {
                    rval +=
                        self.get_coef3(c0, c1, c2)? * sp.spline_value_at_voxel(vox, [c0, c1, c2]);
                }
            }
        }
        Ok(rval)
    }

    /// `Update(fi)`: the field (or a derivative) at every voxel from the coefficients.
    /// (`Update(vector)` computes several at once with the same operations per field.)
    pub fn update(&mut self, fi: FieldIndex) -> Result<(), FslError> {
        if fi as u32 > self.ndim {
            return Err(basisfield_error(
                "splinefield::Update: Cannot take derivative in singleton direction",
            ));
        }
        if self.up_to_date(fi) {
            return Ok(());
        }
        let spline = self.deriv_spline(fi)?;
        let csz = self.coef_size();
        let fsz = self.sz;
        let mut fld = self.field[fi as usize]
            .take()
            .unwrap_or_else(|| vec![0.0; self.field_sz() as usize]);
        get_field(&spline, &self.coef, csz, fsz, &mut fld);
        self.field[fi as usize] = Some(fld);
        self.futd[fi as usize] = true;
        Ok(())
    }

    /// `Get(fi)`: the field (or a derivative) at every voxel, x fastest.
    pub fn get(&mut self, fi: FieldIndex) -> Result<&[f64], FslError> {
        if !self.up_to_date(fi) {
            self.update(fi)?;
        }
        Ok(self.field[fi as usize].as_deref().expect("updated"))
    }

    /// `AsVolume(vol, fi)`: the field (or a derivative) written into `vol` as `float`.
    pub fn as_volume(&mut self, vol: &mut Volume<f32>, fi: FieldIndex) -> Result<(), FslError> {
        if i64::from(self.sz[0]) != vol.xsize()
            || i64::from(self.sz[1]) != vol.ysize()
            || i64::from(self.sz[2]) != vol.zsize()
        {
            return Err(basisfield_error(
                "basisfield::AsVolume:: Matrix size mismatch beween field and volume",
            ));
        }
        if self.vxs[0] != f64::from(vol.xdim())
            || self.vxs[1] != f64::from(vol.ydim())
            || self.vxs[2] != f64::from(vol.zdim())
        {
            return Err(basisfield_error(
                "basisfield::AsVolume:: Voxel size mismatch beween field and volume",
            ));
        }
        let n = self.field_sz() as usize;
        let f = self.get(fi)?;
        for (d, &s) in vol.data[..n].iter_mut().zip(f.iter()) {
            *d = s as f32;
        }
        Ok(())
    }

    /// `Set(pfield)`: the coefficients of the field that fits `pfield` (as `double`) best, one
    /// direction after the other (x, then y and z for fields of that many dimensions): along
    /// each line, `b = M·y` with `M = (ASᵀAS)⁻¹Aᵀ`, `A` the splines sampled at the voxels
    /// (`GetAMatrix`) and `AS = A & 0.005·S`, `S` a penalty on the two ends (`get_s_matrix`).
    /// The voxel sizes are compared as FSL does (the field's `double` with the volume's
    /// `float`).
    pub fn set_from_volume(&mut self, pfield: &Volume<f32>) -> Result<(), FslError> {
        if i64::from(self.sz[0]) != pfield.xsize()
            || i64::from(self.sz[1]) != pfield.ysize()
            || i64::from(self.sz[2]) != pfield.zsize()
        {
            return Err(basisfield_error(
                "basisfield::Set:: Matrix size mismatch beween basisfield class and supplied field",
            ));
        }
        if self.vxs[0] != f64::from(pfield.xdim())
            || self.vxs[1] != f64::from(pfield.ydim())
            || self.vxs[2] != f64::from(pfield.zdim())
        {
            return Err(basisfield_error(
                "basisfield::Set:: Voxel size mismatch beween basisfield class and supplied field",
            ));
        }
        let n = self.field_sz() as usize;
        let mut hv = HelperVol {
            sz: self.sz,
            data: pfield.data[..n].iter().map(|&v| f64::from(v)).collect(),
        };
        let csz = self.coef_size();
        let ksp = self.ksp();
        let order = self.order();
        hv = get_coefs_one_dim(&hv, 0, csz[0], order, ksp[0])?;
        if self.ndim > 1 {
            hv = get_coefs_one_dim(&hv, 1, csz[1], order, ksp[1])?;
        }
        if self.ndim > 2 {
            hv = get_coefs_one_dim(&hv, 2, csz[2], order, ksp[2])?;
        }
        self.set_coef(&hv.data)
    }

    fn check_image(&self, ima: &Volume<f32>, msg: &str) -> Result<(), FslError> {
        if ima.xsize() != i64::from(self.sz[0])
            || ima.ysize() != i64::from(self.sz[1])
            || ima.zsize() != i64::from(self.sz[2])
        {
            return Err(basisfield_error(msg));
        }
        Ok(())
    }

    /// `Jte(deriv, ima1, ima2, mask)`: `Jᵀ(ima1 ∘ ima2)` for the splines differentiated
    /// `deriv` times along each direction (`deriv` must have three elements).
    pub fn jte2(
        &self,
        deriv: &[u32],
        ima1: &Volume<f32>,
        ima2: &Volume<f32>,
        mask: Option<&Volume<i8>>,
    ) -> Result<Vec<f64>, FslError> {
        if deriv.len() != 3 {
            return Err(basisfield_error(
                "splinefield::Jte: Wrong size deriv vector",
            ));
        }
        if !samesize3(ima1, ima2, true) || mask.is_some_and(|m| !samesize3(ima1, m, false)) {
            return Err(basisfield_error(
                "splinefield::Jte: Image dimensionality mismatch",
            ));
        }
        self.check_image(
            ima1,
            "splinefield::Jte: Image-Field dimensionality mismatch",
        )?;
        let prodima = hadamard(ima1, ima2, mask, self.field_sz() as usize);
        let spline = Spline3D::new(
            self.sp.order(),
            self.sp.knot_spacings(),
            [deriv[0], deriv[1], deriv[2]],
        )?;
        let mut ovec = vec![0.0; self.coef_sz() as usize];
        get_jte(&spline, self.coef_size(), &prodima, self.sz, &mut ovec);
        Ok(ovec)
    }

    /// `Jte(ima1, ima2, mask)`: [`Self::jte2`] with no derivative (`deriv` has `NDim()`
    /// elements, so a field of fewer than three dimensions is refused, as FSL refuses it).
    pub fn jte2_field(
        &self,
        ima1: &Volume<f32>,
        ima2: &Volume<f32>,
        mask: Option<&Volume<i8>>,
    ) -> Result<Vec<f64>, FslError> {
        let deriv = vec![0u32; self.ndim as usize];
        self.jte2(&deriv, ima1, ima2, mask)
    }

    /// `Jte(deriv, ima, mask)`: `Jᵀ ima` with the image zeroed outside the mask.
    pub fn jte(
        &self,
        deriv: &[u32],
        ima: &Volume<f32>,
        mask: Option<&Volume<i8>>,
    ) -> Result<Vec<f64>, FslError> {
        if deriv.len() != 3 {
            return Err(basisfield_error(
                "splinefield::Jte: Wrong size if deriv vector",
            ));
        }
        if mask.is_some_and(|m| !samesize3(ima, m, false)) {
            return Err(basisfield_error(
                "splinefield::Jte: Image-Mask dimensionality mismatch",
            ));
        }
        self.check_image(ima, "splinefield::Jte: Image-Field dimensionality mismatch")?;
        let n = self.field_sz() as usize;
        let fima: Vec<f32> = match mask {
            Some(m) => ima.data[..n]
                .iter()
                .zip(m.data[..n].iter())
                .map(|(&v, &k)| if k != 0 { v } else { 0.0 })
                .collect(),
            None => ima.data[..n].to_vec(),
        };
        let spline = Spline3D::new(
            self.sp.order(),
            self.sp.knot_spacings(),
            [deriv[0], deriv[1], deriv[2]],
        )?;
        let mut ovec = vec![0.0; self.coef_sz() as usize];
        get_jte(&spline, self.coef_size(), &fima, self.sz, &mut ovec);
        Ok(ovec)
    }

    /// `Jte(ima, mask)`.
    pub fn jte_field(
        &self,
        ima: &Volume<f32>,
        mask: Option<&Volume<i8>>,
    ) -> Result<Vec<f64>, FslError> {
        let deriv = vec![0u32; self.ndim as usize];
        self.jte(&deriv, ima, mask)
    }

    /// `JtJ(deriv, ima1, ima2, mask, prec)`: `Jᵀ diag(ima1 ∘ ima2) J` for the splines
    /// differentiated `deriv` times (symmetric).
    pub fn jtj(
        &self,
        deriv: &[u32],
        ima1: &Volume<f32>,
        ima2: &Volume<f32>,
        mask: Option<&Volume<i8>>,
    ) -> Result<Csc, FslError> {
        if deriv.len() != 3 {
            return Err(basisfield_error(
                "splinefield::JtJ: Wrong size derivative vector",
            ));
        }
        if !samesize3(ima1, ima2, true) {
            return Err(basisfield_error(
                "splinefield::JtJ: Image dimension mismatch",
            ));
        }
        if mask.is_some_and(|m| !samesize3(ima1, m, false)) {
            return Err(basisfield_error(
                "splinefield::JtJ: Mismatch between image and mask",
            ));
        }
        self.check_image(ima1, "splinefield::JtJ: Mismatch between image and field")?;
        let prodima = hadamard(ima1, ima2, mask, self.field_sz() as usize);
        let csz = self.coef_size();
        if deriv == [0, 0, 0] {
            Ok(make_fully_symmetric_jtj(&self.sp, csz, &prodima, self.sz))
        } else {
            let sp = Spline3D::new(
                self.sp.order(),
                self.sp.knot_spacings(),
                [deriv[0], deriv[1], deriv[2]],
            )?;
            Ok(make_fully_symmetric_jtj(&sp, csz, &prodima, self.sz))
        }
    }

    /// `JtJ(ima, mask, prec)`: `JᵀJ` weighted by `ima ∘ ima`.
    pub fn jtj_field(&self, ima: &Volume<f32>, mask: Option<&Volume<i8>>) -> Result<Csc, FslError> {
        self.jtj(&[0, 0, 0], ima, ima, mask)
    }

    /// `JtJ(deriv1, ima1, deriv2, ima2, mask, prec)`: `J₁ᵀ diag(ima1 ∘ ima2) J₂` (rows from
    /// the splines differentiated `deriv1` times, columns `deriv2`); the symmetric
    /// [`Self::jtj`] when the two are the same.
    pub fn jtj_cross(
        &self,
        deriv1: &[u32],
        ima1: &Volume<f32>,
        deriv2: &[u32],
        ima2: &Volume<f32>,
        mask: Option<&Volume<i8>>,
    ) -> Result<Csc, FslError> {
        if deriv1.len() != 3 || deriv2.len() != 3 {
            return Err(basisfield_error(
                "splinefield::JtJ: Wrong size derivative vector",
            ));
        }
        if deriv1 == deriv2 {
            return self.jtj(deriv1, ima1, ima2, mask);
        }
        if !samesize3(ima1, ima2, true) {
            return Err(basisfield_error(
                "splinefield::JtJ: Image dimension mismatch",
            ));
        }
        if mask.is_some_and(|m| !samesize3(ima1, m, false)) {
            return Err(basisfield_error(
                "splinefield::JtJ: Mismatch between image and mask",
            ));
        }
        self.check_image(ima1, "splinefield::JtJ: Mismatch between image and field")?;
        let prodima = hadamard(ima1, ima2, mask, self.field_sz() as usize);
        let csz = self.coef_size();
        let k = self.sp.knot_spacings();
        let sp1 = Spline3D::new(self.sp.order(), k, [deriv1[0], deriv1[1], deriv1[2]])?;
        let sp2 = Spline3D::new(self.sp.order(), k, [deriv2[0], deriv2[1], deriv2[2]])?;
        Ok(make_asymmetric_jtj(&sp1, csz, &sp2, csz, &prodima, self.sz))
    }

    /// `JtJ(ima1, bf2, ima2, mask, prec)`: rows from this field's splines, columns from
    /// `bf2`'s. FSL compares the two fields' sizes along x and y only (its z test compares this
    /// field with itself).
    pub fn jtj_other(
        &self,
        ima1: &Volume<f32>,
        bf2: &SplineField,
        ima2: &Volume<f32>,
        mask: Option<&Volume<i8>>,
    ) -> Result<Csc, FslError> {
        if !samesize3(ima1, ima2, true) {
            return Err(basisfield_error(
                "splinefield::JtJ: Image dimension mismatch",
            ));
        }
        if mask.is_some_and(|m| !samesize3(ima1, m, false)) {
            return Err(basisfield_error(
                "splinefield::JtJ: Mismatch between image and mask",
            ));
        }
        self.check_image(ima1, "splinefield::JtJ: Mismatch between image and field")?;
        if self.sz[0] != bf2.sz[0] || self.sz[1] != bf2.sz[1] {
            return Err(basisfield_error(
                "splinefield::JtJ: Mismatch between fields",
            ));
        }
        let prodima = hadamard(ima1, ima2, mask, self.field_sz() as usize);
        Ok(make_asymmetric_jtj(
            &self.sp,
            self.coef_size(),
            &bf2.sp,
            bf2.coef_size(),
            &prodima,
            self.sz,
        ))
    }

    /// `MemEnergyGrad()`: `2·AᵀA·b` with `AᵀA` from the products of the first derivatives (in
    /// voxels: FSL's gradient does not scale by the voxel sizes, its Hessian does).
    pub fn mem_energy_grad(&self) -> Result<Vec<f64>, FslError> {
        let lksp = self.ksp();
        let ord = self.order();
        let sp1 = Spline3D::new(ord, lksp, [1, 0, 0])?;
        let mut sum = MemenHelper::new(&sp1)?;
        for d in 1..3 {
            let mut deriv = [0u32; 3];
            deriv[d] = 1;
            let sp2 = Spline3D::new(ord, lksp, deriv)?;
            sum.add(&MemenHelper::new(&sp2)?)?;
        }
        let mut grad = calculate_atab(&self.coef, self.sz, self.coef_size(), &sp1, &sum);
        grad.iter_mut().for_each(|g| *g *= 2.0);
        Ok(grad)
    }

    /// `BendEnergyGrad()`: `2·AᵀA·b` with `AᵀA` from the products of the second derivatives in
    /// mm⁻² (the cross terms twice).
    pub fn bend_energy_grad(&self) -> Result<Vec<f64>, FslError> {
        let lksp = self.ksp();
        let ord = self.order();
        let sp = Spline3D::new(ord, lksp, [0, 0, 0])?;
        let mut sum = MemenHelper::new(&sp)?;
        sum.scale(0.0);
        for d1 in 0..3 {
            for d2 in d1..3 {
                let mut deriv = [0u32; 3];
                deriv[d1] += 1;
                deriv[d2] += 1;
                let mut spd = Spline3D::new(ord, lksp, deriv)?;
                spd.divide(self.vxs[d1] * self.vxs[d2])?;
                let mut h = MemenHelper::new(&spd)?;
                if d1 != d2 {
                    h.scale(2.0);
                }
                sum.add(&h)?;
            }
        }
        let mut grad = calculate_atab(&self.coef, self.sz, self.coef_size(), &sp, &sum);
        grad.iter_mut().for_each(|g| *g *= 2.0);
        Ok(grad)
    }

    /// `MemEnergy()`: `coef · (0.5 · MemEnergyGrad())` (`DotProduct`: Armadillo's `dot`,
    /// OpenBLAS's `ddot` beyond 32 coefficients on `blas_threads` threads).
    pub fn mem_energy(&self, blas_threads: usize) -> Result<f64, FslError> {
        let atab: Vec<f64> = self.mem_energy_grad()?.iter().map(|&g| 0.5 * g).collect();
        Ok(crate::arma::dot_blas(&self.coef, &atab, blas_threads))
    }

    /// `BendEnergy()`: `coef · (0.5 · BendEnergyGrad())`.
    pub fn bend_energy(&self, blas_threads: usize) -> Result<f64, FslError> {
        let atab: Vec<f64> = self.bend_energy_grad()?.iter().map(|&g| 0.5 * g).collect();
        Ok(crate::arma::dot_blas(&self.coef, &atab, blas_threads))
    }

    /// `MemEnergyHess(prec)`: the membrane energy's Hessian (first derivatives in mm⁻¹, twice).
    pub fn mem_energy_hess(&self) -> Result<Csc, FslError> {
        self.memen_bender_h(EnergyType::MemEn)
    }

    /// `BendEnergyHess(prec)`: the bending energy's Hessian.
    pub fn bend_energy_hess(&self) -> Result<Csc, FslError> {
        self.memen_bender_h(EnergyType::BendEn)
    }

    /// `calculate_memen_bender_H` (and its threaded twin, which fills each column alike).
    fn memen_bender_h(&self, et: EnergyType) -> Result<Csc, FslError> {
        let lksp = self.ksp();
        let csz = self.coef_size();
        let isz = self.sz;
        let ord = self.order();
        let vxs = self.vxs;
        let mut helpers: Vec<MemenHelper> = Vec::with_capacity(6);
        match et {
            EnergyType::MemEn => {
                for d in 0..3 {
                    let mut deriv = [0u32; 3];
                    deriv[d] = 1;
                    let mut spd = Spline3D::new(ord, lksp, deriv)?;
                    spd.divide(vxs[d])?;
                    let mut h = MemenHelper::new(&spd)?;
                    h.scale(2.0);
                    helpers.push(h);
                }
            }
            EnergyType::BendEn => {
                for d1 in 0..3 {
                    for d2 in d1..3 {
                        let mut deriv = [0u32; 3];
                        deriv[d1] += 1;
                        deriv[d2] += 1;
                        let mut spd = Spline3D::new(ord, lksp, deriv)?;
                        spd.divide(vxs[d1] * vxs[d2])?;
                        let mut h = MemenHelper::new(&spd)?;
                        h.scale(if d1 == d2 { 2.0 } else { 4.0 });
                        helpers.push(h);
                    }
                }
            }
        }
        let sp = Spline3D::new(ord, lksp, [0, 0, 0])?;
        let ncoef = csz[0] * csz[1] * csz[2];
        let mut irp = Vec::with_capacity(sp.nz_max(isz) as usize);
        let mut val = Vec::with_capacity(irp.capacity());
        let mut jcp = vec![0u32; ncoef as usize + 1];
        let mut fo = [0u32; 3];
        let mut lo = [0u32; 3];
        let mut ci = 0u32;
        for c2 in 0..csz[2] {
            for c1 in 0..csz[1] {
                for c0 in 0..csz[0] {
                    ci = c2 * csz[1] * csz[0] + c1 * csz[0] + c0;
                    jcp[ci as usize] = irp.len() as u32;
                    sp.range_of_overlapping_splines([c0, c1, c2], isz, &mut fo, &mut lo);
                    for k in fo[2]..lo[2] {
                        for j in fo[1]..lo[1] {
                            let bi = k * csz[1] * csz[0] + j * csz[0];
                            for i in fo[0]..lo[0] {
                                irp.push(bi + i);
                                let mut v = 0.0;
                                for h in &helpers {
                                    v += h.peek(
                                        i.wrapping_sub(c0) as i32,
                                        j.wrapping_sub(c1) as i32,
                                        k.wrapping_sub(c2) as i32,
                                    );
                                }
                                val.push(v);
                            }
                        }
                    }
                }
            }
        }
        jcp[ci as usize + 1] = irp.len() as u32;
        Ok(Csc {
            m: ncoef,
            n: ncoef,
            irp,
            jcp,
            val,
        })
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum EnergyType {
    MemEn,
    BendEn,
}

/// `hadamard(ima1, ima2, mask, prod)`: `ima1·ima2` in `float`, with a mask `(mask·ima1)·ima2`.
fn hadamard(
    ima1: &Volume<f32>,
    ima2: &Volume<f32>,
    mask: Option<&Volume<i8>>,
    n: usize,
) -> Vec<f32> {
    match mask {
        Some(m) => (0..n)
            .map(|i| f32::from(m.data[i]) * ima1.data[i] * ima2.data[i])
            .collect(),
        None => (0..n).map(|i| ima1.data[i] * ima2.data[i]).collect(),
    }
}

/// `get_field(sp, coef, csz, fsz, fld)`: each coefficient times its spline added into the
/// voxels it covers, coefficients in storage order.
pub fn get_field(sp: &Spline3D, coef: &[f64], csz: [u32; 3], fsz: [u32; 3], fld: &mut [f64]) {
    let ks = [sp.kernel_size(0), sp.kernel_size(1), sp.kernel_size(2)];
    fld.iter_mut().for_each(|v| *v = 0.0);
    for c2 in 0..csz[2] {
        for c1 in 0..csz[1] {
            for c0 in 0..csz[0] {
                let ci = [c0, c1, c2];
                let (ff, lf) = sp.range_in_field(ci, fsz);
                let os = sp.offset_into_kernel(ci, fsz);
                let c = coef[(c2 * csz[1] * csz[0] + c1 * csz[0] + c0) as usize];
                let mut sk = os[2];
                for fk in ff[2]..lf[2] {
                    let mut sj = os[1];
                    for fj in ff[1]..lf[1] {
                        let fbi = fk * fsz[1] * fsz[0] + fj * fsz[0];
                        let sbi = sk * ks[1] * ks[0] + sj * ks[0];
                        let mut si = os[0];
                        for fi in ff[0]..lf[0] {
                            fld[(fbi + fi) as usize] += c * sp.at(sbi + si);
                            si += 1;
                        }
                        sj += 1;
                    }
                    sk += 1;
                }
            }
        }
    }
}

/// `get_jte(sp, csz, ima, isz, jte)`: for each coefficient, the sum over its spline's voxels of
/// the kernel times the image (z, then y, then x).
fn get_jte(sp: &Spline3D, csz: [u32; 3], ima: &[f32], isz: [u32; 3], jte: &mut [f64]) {
    let ks = [sp.kernel_size(0), sp.kernel_size(1), sp.kernel_size(2)];
    for c2 in 0..csz[2] {
        for c1 in 0..csz[1] {
            for c0 in 0..csz[0] {
                let ci = [c0, c1, c2];
                let (fi, li) = sp.range_in_field(ci, isz);
                let os = sp.offset_into_kernel(ci, isz);
                let mut acc = 0.0f64;
                let mut sk = os[2];
                for ik in fi[2]..li[2] {
                    let mut sj = os[1];
                    for ij in fi[1]..li[1] {
                        let ibi = ik * isz[1] * isz[0] + ij * isz[0];
                        let sbi = sk * ks[1] * ks[0] + sj * ks[0];
                        let mut si = os[0];
                        for ii in fi[0]..li[0] {
                            acc += sp.at(sbi + si) * f64::from(ima[(ibi + ii) as usize]);
                            si += 1;
                        }
                        sj += 1;
                    }
                    sk += 1;
                }
                jte[(c2 * csz[1] * csz[0] + c1 * csz[0] + c0) as usize] = acc;
            }
        }
    }
}

/// `ZeroSplineMap`: for each spline, whether the image is exactly 0 over all its voxels.
struct ZeroSplineMap {
    csz: [u32; 3],
    map: Vec<bool>,
}

impl ZeroSplineMap {
    fn new(sp: &Spline3D, csz: [u32; 3], ima: &[f32], isz: [u32; 3]) -> ZeroSplineMap {
        let mut map = vec![false; (csz[0] * csz[1] * csz[2]) as usize];
        for c2 in 0..csz[2] {
            for c1 in 0..csz[1] {
                let bi = c2 * csz[1] * csz[0] + c1 * csz[0];
                for c0 in 0..csz[0] {
                    let (first, last) = sp.range_in_field([c0, c1, c2], isz);
                    map[(bi + c0) as usize] = all_zeros(first, last, ima, isz);
                }
            }
        }
        ZeroSplineMap { csz, map }
    }

    fn is_zero(&self, c: [u32; 3]) -> bool {
        self.map[((c[2] * self.csz[1] + c[1]) * self.csz[0] + c[0]) as usize]
    }
}

fn all_zeros(first: [u32; 3], last: [u32; 3], ima: &[f32], isz: [u32; 3]) -> bool {
    for k in first[2]..last[2] {
        for j in first[1]..last[1] {
            let bi = k * isz[1] * isz[0] + j * isz[0];
            for i in first[0]..last[0] {
                if ima[(bi + i) as usize] != 0.0 {
                    return false;
                }
            }
        }
    }
    true
}

/// `get_val(row, col, irp, jcp, val)`: binary search of `row` in column `col` (0 when absent).
fn get_val(row: u32, col: u32, irp: &[u32], jcp: &[u32], val: &[f64]) -> f64 {
    let start = jcp[col as usize] as usize;
    let n = (jcp[col as usize + 1] - jcp[col as usize]) as i32;
    let a = &irp[start..];
    let v = &val[start..];
    if n == 0 || row < a[0] || row > a[(n - 1) as usize] {
        return 0.0;
    }
    let (mut jlo, mut jup) = (-1i32, n);
    while jup - jlo > 1 {
        let j = (jlo + jup) >> 1;
        if row >= a[j as usize] {
            jlo = j;
        } else {
            jup = j;
        }
    }
    if a[jlo as usize] == row {
        v[jlo as usize]
    } else {
        0.0
    }
}

/// `make_fully_symmetric_jtj(sp2, csz, ima, isz, prec)` (one thread): the elements whose row
/// index is at least the column's along every direction computed (`MulByOther` of the
/// pre-multiplied column spline and the row spline), every other element copied from the
/// column already made that holds the same pair of splines.
fn make_fully_symmetric_jtj(sp2: &Spline3D, csz: [u32; 3], ima: &[f32], isz: [u32; 3]) -> Csc {
    let mut sp1 = sp2.copy();
    let mut fo = [0u32; 3];
    let mut lo = [0u32; 3];
    let ncoef = csz[2] * csz[1] * csz[0];
    let cap = sp1.nz_max(isz) as usize;
    let mut irp: Vec<u32> = Vec::with_capacity(cap);
    let mut val: Vec<f64> = Vec::with_capacity(cap);
    let mut jcp = vec![0u32; ncoef as usize + 1];
    let mut ci = 0u32;
    let zeromap = ZeroSplineMap::new(&sp1, csz, ima, isz);
    let lin = |c: [u32; 3]| c[2] * csz[1] * csz[0] + c[1] * csz[0] + c[0];
    for x2 in 0..csz[2] {
        for x1 in 0..csz[1] {
            for x0 in 0..csz[0] {
                let indx1 = [x0, x1, x2];
                ci = lin(indx1);
                jcp[ci as usize] = irp.len() as u32;
                let indx1_is_zero = zeromap.is_zero(indx1);
                if !indx1_is_zero {
                    sp1.premul(indx1, isz, ima);
                }
                sp1.range_of_overlapping_splines(indx1, isz, &mut fo, &mut lo);
                for k in fo[2]..x2 {
                    for j in fo[1]..lo[1] {
                        for i in fo[0]..lo[0] {
                            let ri = lin([i, j, k]);
                            irp.push(ri);
                            let v = if indx1_is_zero {
                                0.0
                            } else {
                                get_val(ci, ri, &irp, &jcp, &val)
                            };
                            val.push(v);
                        }
                    }
                }
                for k in x2..lo[2] {
                    for j in fo[1]..x1 {
                        for i in fo[0]..lo[0] {
                            let ri = lin([i, j, k]);
                            let cri = lin([i, j, x2]);
                            let cci = lin([x0, x1, k]);
                            irp.push(ri);
                            let v = if indx1_is_zero {
                                0.0
                            } else {
                                get_val(cci, cri, &irp, &jcp, &val)
                            };
                            val.push(v);
                        }
                    }
                    for j in x1..lo[1] {
                        for i in fo[0]..x0 {
                            let ri = lin([i, j, k]);
                            let cri = lin([i, x1, x2]);
                            let cci = lin([x0, j, k]);
                            irp.push(ri);
                            let v = if indx1_is_zero {
                                0.0
                            } else {
                                get_val(cci, cri, &irp, &jcp, &val)
                            };
                            val.push(v);
                        }
                        for i in x0..lo[0] {
                            let ri = lin([i, j, k]);
                            irp.push(ri);
                            let v = if indx1_is_zero || zeromap.is_zero([i, j, k]) {
                                0.0
                            } else {
                                sp1.mul_by_other(indx1, [i, j, k], isz, sp2)
                            };
                            val.push(v);
                        }
                    }
                }
            }
        }
    }
    jcp[ci as usize + 1] = irp.len() as u32;
    Csc {
        m: ncoef,
        n: ncoef,
        irp,
        jcp,
        val,
    }
}

/// `make_asymmetric_jtj(rsp, cszr, sp2, cszc, ima, isz, prec)` (one thread): every element of
/// each column computed (`MulByOther` of the pre-multiplied column spline and the row spline).
fn make_asymmetric_jtj(
    rsp: &Spline3D,
    cszr: [u32; 3],
    sp2: &Spline3D,
    cszc: [u32; 3],
    ima: &[f32],
    isz: [u32; 3],
) -> Csc {
    let mut csp = sp2.copy();
    let mut fo = [0u32; 3];
    let mut lo = [0u32; 3];
    let m = cszr[2] * cszr[1] * cszr[0];
    let n = cszc[2] * cszc[1] * cszc[0];
    let mut irp: Vec<u32> = Vec::new();
    let mut val: Vec<f64> = Vec::new();
    let mut jcp = vec![0u32; n as usize + 1];
    let mut ci = 0u32;
    let r_zeromap = ZeroSplineMap::new(rsp, cszr, ima, isz);
    let c_zeromap = ZeroSplineMap::new(&csp, cszc, ima, isz);
    let sks = (0..3).all(|d| rsp.kernel_size(d) == csp.kernel_size(d));
    for x2 in 0..cszc[2] {
        for x1 in 0..cszc[1] {
            for x0 in 0..cszc[0] {
                let cindx = [x0, x1, x2];
                ci = x2 * cszc[1] * cszc[0] + x1 * cszc[0] + x0;
                jcp[ci as usize] = irp.len() as u32;
                let c_is_zero = c_zeromap.is_zero(cindx);
                if !c_is_zero {
                    csp.premul(cindx, isz, ima);
                }
                if sks {
                    csp.range_of_overlapping_splines(cindx, isz, &mut fo, &mut lo);
                } else {
                    csp.range_of_overlapping_splines_with(cindx, isz, rsp, &mut fo, &mut lo);
                }
                for k in fo[2]..lo[2] {
                    for j in fo[1]..lo[1] {
                        for i in fo[0]..lo[0] {
                            irp.push(k * cszr[1] * cszr[0] + j * cszr[0] + i);
                            let v = if c_is_zero || r_zeromap.is_zero([i, j, k]) {
                                0.0
                            } else {
                                csp.mul_by_other(cindx, [i, j, k], isz, rsp)
                            };
                            val.push(v);
                        }
                    }
                }
            }
        }
    }
    jcp[ci as usize + 1] = irp.len() as u32;
    Csc {
        m,
        n,
        irp,
        jcp,
        val,
    }
}

/// `Memen_H_Helper`: the products of one spline with its neighbours (`MulByOther` in a field of
/// 1000 knot spacings, around its middle spline), indexed by the offset between the two.
#[derive(Clone, Debug)]
struct MemenHelper {
    sz: [i32; 3],
    cntr: [i32; 3],
    data: Vec<f64>,
}

impl MemenHelper {
    fn new(sp: &Spline3D) -> Result<MemenHelper, FslError> {
        let mut isz = [0u32; 3];
        let mut cindx = [0u32; 3];
        for i in 0..3 {
            isz[i] = 1000 * sp.knot_spacing(i);
            cindx[i] = sp.ncoef(i, isz[i]) / 2;
        }
        let mut first = [0u32; 3];
        let mut last = [0u32; 3];
        if !sp.range_of_overlapping_splines(cindx, isz, &mut first, &mut last) {
            return Err(basisfield_error(
                "Memen_H_Helper::Memen_H_Helper: No overlapping splines",
            ));
        }
        let sz = [
            (last[0] - first[0]) as i32,
            (last[1] - first[1]) as i32,
            (last[2] - first[2]) as i32,
        ];
        let cntr = [
            (cindx[0] - first[0]) as i32,
            (cindx[1] - first[1]) as i32,
            (cindx[2] - first[2]) as i32,
        ];
        let mut data = vec![0.0; (sz[0] * sz[1] * sz[2]) as usize];
        for ck in first[2]..last[2] {
            for cj in first[1]..last[1] {
                for c in first[0]..last[0] {
                    let li = (ck as i32 - cindx[2] as i32 + cntr[2]) * sz[1] * sz[0]
                        + (cj as i32 - cindx[1] as i32 + cntr[1]) * sz[0]
                        + (c as i32 - cindx[0] as i32 + cntr[0]);
                    data[li as usize] = sp.mul_by_other(cindx, [c, cj, ck], isz, sp);
                }
            }
        }
        Ok(MemenHelper { sz, cntr, data })
    }

    #[inline]
    fn peek(&self, i: i32, j: i32, k: i32) -> f64 {
        self.data[((k + self.cntr[2]) * self.sz[1] * self.sz[0]
            + (j + self.cntr[1]) * self.sz[0]
            + (i + self.cntr[0])) as usize]
    }

    fn scale(&mut self, s: f64) {
        self.data.iter_mut().for_each(|v| *v *= s);
    }

    fn add(&mut self, rhs: &MemenHelper) -> Result<(), FslError> {
        if self.sz != rhs.sz {
            return Err(basisfield_error("Memen_H_Helper::+=(): Size mismatch"));
        }
        for (a, b) in self.data.iter_mut().zip(rhs.data.iter()) {
            *a += *b;
        }
        Ok(())
    }
}

/// `calculate_AtAb(b, isz, csz, sp, hlpr, grad)`: for each coefficient, the helper's products
/// with its overlapping neighbours times their coefficients, z, then y, then x.
fn calculate_atab(
    b: &[f64],
    isz: [u32; 3],
    csz: [u32; 3],
    sp: &Spline3D,
    hlpr: &MemenHelper,
) -> Vec<f64> {
    let mut grad = vec![0.0f64; b.len()];
    let mut first = [0u32; 3];
    let mut last = [0u32; 3];
    let mut li: i64 = 0;
    let (c0s, c1s) = (i64::from(csz[0]), i64::from(csz[1]));
    for c2 in 0..csz[2] {
        for c1 in 0..csz[1] {
            for c0 in 0..csz[0] {
                sp.range_of_overlapping_splines([c0, c1, c2], isz, &mut first, &mut last);
                let lastk = i64::from(last[2]) - i64::from(c2);
                let lastj = i64::from(last[1]) - i64::from(c1);
                let lasti = i64::from(last[0]) - i64::from(c0);
                let mut acc = 0.0f64;
                for k in (i64::from(first[2]) - i64::from(c2))..lastk {
                    let offset = k * c1s * c0s;
                    for j in (i64::from(first[1]) - i64::from(c1))..lastj {
                        let offset2 = li + offset + j * c0s;
                        for i in (i64::from(first[0]) - i64::from(c0))..lasti {
                            acc +=
                                hlpr.peek(i as i32, j as i32, k as i32) * b[(offset2 + i) as usize];
                        }
                    }
                }
                grad[li as usize] = acc;
                li += 1;
            }
        }
    }
    grad
}

/// `helper_vol`: a `double` volume whose lines along any direction can be read and written.
struct HelperVol {
    sz: [u32; 3],
    data: Vec<f64>,
}

impl HelperVol {
    /// `start(i, j, dim)` and `step(dim)`.
    fn line(&self, i: u32, j: u32, dim: usize) -> (usize, usize) {
        let (s0, s1) = (self.sz[0] as usize, self.sz[1] as usize);
        let (i, j) = (i as usize, j as usize);
        match dim {
            0 => (j * s0 * s1 + i * s0, 1),
            1 => (j * s0 * s1 + i, s0),
            _ => (j * s0 + i, s0 * s1),
        }
    }
    fn get_column(&self, i: u32, j: u32, dim: usize) -> Vec<f64> {
        let (st, step) = self.line(i, j, dim);
        (0..self.sz[dim] as usize)
            .map(|k| self.data[st + k * step])
            .collect()
    }
    fn set_column(&mut self, i: u32, j: u32, dim: usize, col: &[f64]) {
        let (st, step) = self.line(i, j, dim);
        for (k, &v) in col.iter().enumerate().take(self.sz[dim] as usize) {
            self.data[st + k * step] = v;
        }
    }
}

/// `Spline1D::GetAMatrix(isz, csz)`: the `isz × csz` matrix of the splines sampled at the
/// voxels (column `i`: spline `i` over its range).
fn get_a_matrix(sp: &Spline1D, isz: u32, csz: u32) -> Mat {
    let csz = if csz == 0 { sp.ncoef(isz) } else { csz };
    let mut a = Mat::zeros(isz as usize, csz as usize);
    for i in 0..csz {
        let (fr, lr) = sp.range(i, isz);
        let so = sp.offset(i, isz);
        for (si, j) in (so..).zip(fr..lr) {
            a.set(j as usize, i as usize, sp.at(si));
        }
    }
    a
}

/// `get_s_matrix(isz, csz)`: zeros but `2, −1, …, −1` in the first row and `−1, …, −1, 2` in
/// the last (assigned in that order, so a single row keeps the last row's values where they
/// meet).
fn get_s_matrix(isz: u32, csz: u32) -> Mat {
    let (r, c) = (isz as usize, csz as usize);
    let mut s = Mat::zeros(r, c);
    s.set(0, 0, 2.0);
    s.set(0, 1, -1.0);
    s.set(0, c - 1, -1.0);
    s.set(r - 1, 0, -1.0);
    s.set(r - 1, c - 2, -1.0);
    s.set(r - 1, c - 1, 2.0);
    s
}

/// `get_coefs_one_dim(in, dim, csz, order, ksp)`: every line of `in` along `dim` replaced by
/// its `csz` coefficients.
fn get_coefs_one_dim(
    inv: &HelperVol,
    dim: usize,
    csz: u32,
    order: u32,
    ksp: u32,
) -> Result<HelperVol, FslError> {
    let mut nsz = inv.sz;
    nsz[dim] = csz;
    let mut out = HelperVol {
        sz: nsz,
        data: vec![0.0; nsz.iter().map(|&v| v as usize).product()],
    };
    let (ii, jj) = match dim {
        0 => (inv.sz[1], inv.sz[2]),
        1 => (inv.sz[0], inv.sz[2]),
        _ => (inv.sz[0], inv.sz[1]),
    };
    let isz = inv.sz[dim];
    if csz < 2 {
        // get_s_matrix writes column 2 (FSL: outside a one-column matrix); never met (a spline
        // field has at least 3 coefficients per direction).
        return Err(basisfield_error(
            "splinefield::get_coefs_one_dim: fewer than 2 coefficients",
        ));
    }
    let sp = Spline1D::new(order, ksp, 0)?;
    let a = get_a_matrix(&sp, isz, csz);
    let s = get_s_matrix(isz, csz).scale(0.005);
    let as_ = crate::bfmatrix::vcat(&a, &s);
    let ata = crate::arma::syrk_t(&as_);
    let m = crate::arma::solve(&ata, &a.t()).map_err(|_| FslError {
        printed: Vec::new(),
        type_name: "std::runtime_error",
        what: "matrix multiplication: problem with matrix inverse; suggest to use solve() instead"
            .to_owned(),
    })?;
    for i in 0..ii {
        for j in 0..jj {
            let y = inv.get_column(i, j, dim);
            let b = crate::arma::mul(&m, &Mat::col(&y), false);
            out.set_column(i, j, dim, b.as_slice());
        }
    }
    Ok(out)
}
