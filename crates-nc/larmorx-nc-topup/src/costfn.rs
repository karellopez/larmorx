// SPDX-License-Identifier: LicenseRef-FSL
// Translated to Rust from FSL 6.0.7.23 (fsl-topup 2203.5: topup_costfunctions.{h,cpp}, the
// class TopupCF). Copyright (C) 2012 University of Oxford (Jesper Andersson). Rust translation
// Copyright 2026 Karel Lopez Vilaret; modified from the originals. Distributed under the FSL
// Licence (see LICENSE).
//! topup's cost function (`TopupCF`): the sum of squared differences between each resampled
//! scan and their mean inside the common mask, over `n (m - 1)`, plus `λ` (times the latest
//! SSD with `--ssqlambda`) times the field's bending or membrane energy; its gradient and its
//! Gauss-Newton Hessian (sparse, of the precision `--numprec` asks for) with respect to the
//! field's spline coefficients and the movement parameters. Also every file topup writes from
//! it (the coefficients, the field, the corrected scans, the movement parameters and matrices,
//! displacement fields, Jacobians, and the debug files of `--debug`).

use larmorx_core::defects::{self, UpstreamDefects};
use larmorx_nc_core::arma::Mat;
use larmorx_nc_core::basisfield::SplineField;
use larmorx_nc_core::bfmatrix::{BFMatrix, Precision};
use larmorx_nc_core::cnum;
use larmorx_nc_core::nonlin::NonlinCf;
use larmorx_nc_core::settings::Settings;
use larmorx_nc_core::volume::{self, Elem, FslError, SaveError, SaveOptions, Volume};

use crate::fileio::{self, write_ascii_matrix};
use crate::scan::{InterpType, ScanManager, SizeType, topup_error};
use crate::{Console, ExitNow, vol};

/// `RegularisationType`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Regularisation {
    MembraneEnergy,
    BendingEnergy,
}

/// The thread count FSL's single-thread sparse matrices take (`Utilities::NoOfThreads()`'s
/// default) and every sparse product here: the replica computes FSL's one-thread result.
const SPMAT_THREADS: u32 = 1;

/// The knot spacing for warp resolution `wr` (mm) and voxel size `vxs`: `round(wr / vxs)`, at
/// least 1. A negative rounded spacing, which FSL converts to about four billion voxels
/// (kernels no memory holds: FSL-124), is refused in both modes, where FSL meets it.
fn knot_spacing(wr: f64, vxs: &[f64; 3]) -> [u32; 3] {
    [0, 1, 2].map(|i| {
        let r = wr / vxs[i];
        let k = if r > 0.0 {
            (r + 0.5) as i32
        } else {
            (r - 0.5) as i32
        };
        if k < 0 {
            std::panic::resume_unwind(Box::new(ExitNow {
                code: 1,
                printed: vec![format!(
                    "larmorx-nc: error: topup: --warpres {}: a negative warp resolution gives a \
                     knot spacing of {} voxels (FSL-124), kernels no memory holds",
                    cnum::stream(wr),
                    k as u32
                )],
            }));
        }
        if k != 0 { k as u32 } else { 1 }
    })
}

/// `TopupCF`.
pub struct TopupCf<'a, 'w> {
    pub sm: ScanManager,
    pub field: SplineField,
    wr: f64,
    lambda: f64,
    ssql: bool,
    rt: Regularisation,
    mf: bool,
    hp: Precision,
    nthr: u32,
    verb: bool,
    tp: bool,
    dl: u32,
    lssd: f64,
    level: u32,
    iter: u32,
    attempt: u32,
    con: &'a Console<'w>,
    fsl: &'a Settings,
    mode: UpstreamDefects,
}

impl<'a, 'w> TopupCf<'a, 'w> {
    /// `TopupCF(scans, pevecs, rotimes, warpres, sporder, nthreads)`: the field of the scans'
    /// grid and knot spacing `warpres / voxel size`, all 0; `λ` 10 weighted by the SSD, bending
    /// energy, movements estimated, double precision, linear interpolation.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        scans: &Volume,
        pevecs: &[[f64; 3]],
        rotimes: &[f64],
        warpres: f64,
        sporder: u32,
        nthr: u32,
        con: &'a Console<'w>,
        fsl: &'a Settings,
        mode: UpstreamDefects,
    ) -> Result<TopupCf<'a, 'w>, FslError> {
        let sm = ScanManager::new(scans, pevecs, rotimes, nthr)?;
        let size = [
            scans.xsize() as u32,
            scans.ysize() as u32,
            scans.zsize() as u32,
        ];
        let vxs = [
            f64::from(scans.xdim()),
            f64::from(scans.ydim()),
            f64::from(scans.zdim()),
        ];
        let ksp = knot_spacing(warpres, &vxs);
        let field = SplineField::new(&size, &vxs, &ksp, sporder as i32, nthr)?;
        let mut cf = TopupCf {
            sm,
            field,
            wr: warpres,
            lambda: 10.0,
            ssql: true,
            rt: Regularisation::BendingEnergy,
            mf: false,
            hp: Precision::Double,
            nthr,
            verb: false,
            tp: false,
            dl: 0,
            lssd: 0.0,
            level: 0,
            iter: 0,
            attempt: 0,
            con,
            fsl,
            mode,
        };
        cf.sm.set_interpolation_model(con, InterpType::Linear);
        Ok(cf)
    }

    /// `NPar()`.
    pub fn npar(&self) -> usize {
        self.ndefpar() + self.nmovpar()
    }

    /// `NDefPar()`.
    pub fn ndefpar(&self) -> usize {
        self.field.coef_sz() as usize
    }

    /// `NMovPar()`.
    pub fn nmovpar(&self) -> usize {
        if self.mf {
            0
        } else {
            self.sm.n_movement_parameters()
        }
    }

    /// `Par()`: the coefficients, then the movement parameters being estimated.
    pub fn par(&self) -> Vec<f64> {
        let mut v = self.field.coefs().to_vec();
        if !self.mf {
            v.extend(self.sm.movement_parameters(self.con));
        }
        v
    }

    /// `Lambda()`: `λ`, times the latest SSD with `--ssqlambda`.
    pub fn lambda_now(&self) -> f64 {
        if self.ssql {
            self.lssd * self.lambda
        } else {
            self.lambda
        }
    }

    pub fn set_trace_print(&mut self, val: bool) {
        self.tp = val;
        self.sm.set_trace_print(val);
    }

    pub fn set_verbose(&mut self, val: bool) {
        self.verb = val;
    }

    /// `SetDebug(level)`.
    pub fn set_debug(&mut self, level: u32) -> Result<(), FslError> {
        if level > 3 {
            return Err(topup_error("TopupCF::SetDebug: Debug level out of range"));
        }
        self.dl = level;
        Ok(())
    }

    /// `SetLevel(level)`.
    pub fn set_level(&mut self, level: u32) {
        self.level = level;
        self.iter = 0;
        self.attempt = 0;
    }

    pub fn set_interpolation_model(&mut self, it: InterpType) {
        self.sm.set_interpolation_model(self.con, it);
    }

    /// `SetRegridding(ims)`.
    pub fn set_regridding(&mut self, ims: [u32; 3]) -> Result<(), FslError> {
        let cur = self.sm.image_size(self.con, SizeType::Regridded)?;
        if ims != cur {
            self.sm.re_grid(self.con, ims)?;
        }
        Ok(())
    }

    /// `Smooth(fwhm)`.
    pub fn smooth(&mut self, fwhm: f64) {
        if fwhm != self.sm.fwhm() {
            self.sm.smooth(self.con, fwhm);
        }
    }

    /// The field of the target grid at warp resolution `self.wr`.
    fn new_field(&self) -> Result<(SplineField, Volume), FslError> {
        let size = self.sm.image_size(self.con, SizeType::Target)?;
        let vxs = self.sm.image_vxs(self.con, SizeType::Target)?;
        let ksp = knot_spacing(self.wr, &vxs);
        let f = SplineField::new(&size, &vxs, &ksp, self.field.order() as i32, self.nthr)?;
        let v = vol::grid(
            i64::from(size[0]),
            i64::from(size[1]),
            i64::from(size[2]),
            [vxs[0] as f32, vxs[1] as f32, vxs[2] as f32],
        );
        Ok((f, v))
    }

    /// `SubSample(ss)`: the scans subsampled, the field fitted on the new grid from its values
    /// at the new voxels' centres.
    pub fn subsample(&mut self, ss: u32) -> Result<(), FslError> {
        self.con.trace(self.tp, "Entering TopupCF::SubSample");
        if ss != self.sm.subsampling() {
            let step = f64::from(ss) / f64::from(self.sm.subsampling());
            self.sm.subsample(self.con, ss)?;
            let (mut nf, mut v) = self.new_field()?;
            let start = (step - 1.0) / 2.0;
            let mut kk = start;
            for k in 0..v.zsize() {
                let mut jj = start;
                for j in 0..v.ysize() {
                    let mut ii = start;
                    for i in 0..v.xsize() {
                        let val = self.field.peek(
                            ii,
                            jj,
                            kk,
                            larmorx_nc_core::basisfield::FieldIndex::Field,
                        )?;
                        v.set(i, j, k, val as f32);
                        ii += step;
                    }
                    jj += step;
                }
                kk += step;
            }
            nf.set_from_volume(&v)?;
            self.field = nf;
        }
        self.con.trace(self.tp, "Leaving TopupCF::SubSample");
        Ok(())
    }

    /// `SetWarpResolution(wr)` (mm): the field fitted at the new knot spacing.
    pub fn set_warp_resolution(&mut self, wr: f64) -> Result<(), FslError> {
        self.con
            .trace(self.tp, "Entering TopupCF::SetWarpResolution");
        if wr != self.wr {
            self.wr = wr;
            self.sm.field_updated(self.con);
            let (mut nf, mut v) = self.new_field()?;
            self.field
                .as_volume(&mut v, larmorx_nc_core::basisfield::FieldIndex::Field)?;
            nf.set_from_volume(&v)?;
            self.field = nf;
        }
        self.con
            .trace(self.tp, "Leaving TopupCF::SetWarpResolution");
        Ok(())
    }

    pub fn set_movements_fixed(&mut self, val: bool) {
        self.mf = val;
    }

    pub fn set_regularisation(&mut self, lambda: f64, rt: Regularisation) {
        self.lambda = lambda;
        self.rt = rt;
    }

    pub fn set_ssq_lambda(&mut self, val: bool) {
        self.ssql = val;
    }

    pub fn set_hessian_precision(&mut self, hp: Precision) {
        self.hp = hp;
    }

    fn debug_string(&self) -> String {
        format!(
            "level{:02}_iter{:02}_attempt{:02}",
            self.level, self.iter, self.attempt
        )
    }

    // -------------------------------------------------------------------------------------
    // Writing

    /// `save_volume` (and `save_orig_volume`: topup's volumes are never swapped back) with
    /// newimage's description; an `FSLOUTPUTTYPE` FSL cannot write ends the program there.
    pub fn save<T: Elem>(&self, v: &Volume<T>, name: &str) -> Result<(), FslError> {
        save(v, name, self.fsl, self.mode)
    }

    /// `WriteCoefficients(fname)`.
    pub fn write_coefficients(&self, fname: &str) -> Result<(), FslError> {
        self.con
            .trace(self.tp, "Entering TopupCF::WriteCoefficients");
        let coefs = fileio::coefficient_volume(&self.field)?;
        self.save(&coefs, fname)?;
        self.con
            .trace(self.tp, "Leaving TopupCF::WriteCoefficients");
        Ok(())
    }

    /// `WriteUnwarped(fname)` (`--debug`): the corrected scans on the target grid.
    fn write_unwarped_debug(&mut self, fname: &str) -> Result<(), FslError> {
        self.con.trace(self.tp, "Entering TopupCF::WriteUnwarped");
        let sz = self.sm.image_size(self.con, SizeType::Target)?;
        let ns = self.sm.n_scans();
        let mut out = Volume::new(
            i64::from(sz[0]),
            i64::from(sz[1]),
            i64::from(sz[2]),
            ns as i64,
        );
        for i in 0..ns {
            let s = self.sm.get_scan(self.con, i, &mut self.field)?;
            out.set_sub_volume(i as i64, &s)?;
        }
        self.save(&out, fname)?;
        self.con.trace(self.tp, "Leaving TopupCF::WriteUnwarped");
        Ok(())
    }

    /// `WriteUnwarped(fname, hdr, sf)`: the corrected scans in `hdr`'s image, scaled by `sf`
    /// (`float`), 0 outside the mask.
    pub fn write_unwarped(&mut self, fname: &str, hdr: &Volume, sf: f64) -> Result<(), FslError> {
        self.con.trace(self.tp, "Entering TopupCF::WriteUnwarped");
        let mut out = hdr.clone();
        for i in 0..self.sm.n_scans() {
            let mut s = self.sm.get_scan(self.con, i, &mut self.field)?;
            vol::scale(&mut s, sf as f32);
            out.set_sub_volume(i as i64, &s)?;
        }
        let mask = self.sm.mask(self.con, &mut self.field)?.clone();
        let n = mask.data.len();
        for t in 0..out.tsize() as usize {
            for (k, &m) in mask.data.iter().enumerate() {
                if m == 0 {
                    out.data[t * n + k] = 0.0;
                }
            }
        }
        out.copy_properties(hdr);
        self.save(&out, fname)?;
        self.con.trace(self.tp, "Leaving TopupCF::WriteUnwarped");
        Ok(())
    }

    /// `WriteMeanUnwarped(fname, hdr, sf)`: the mean of the corrected scans (scaled by `sf`)
    /// in the mask, the first scan alone outside it, with `hdr`'s properties.
    pub fn write_mean_unwarped(
        &mut self,
        fname: &str,
        hdr: &Volume,
        sf: f64,
    ) -> Result<(), FslError> {
        self.con
            .trace(self.tp, "Entering TopupCF::WriteMeanUnwarped");
        let sff = sf as f32;
        let ns = self.sm.n_scans();
        let mut tmp = hdr.clone();
        for s in 1..ns {
            let mut v = self.sm.get_scan(self.con, s, &mut self.field)?;
            vol::scale(&mut v, sff);
            tmp.set_sub_volume(s as i64, &v)?;
        }
        let mut out = self.sm.get_scan(self.con, 0, &mut self.field)?;
        vol::scale(&mut out, sff);
        let mask = self.sm.mask(self.con, &mut self.field)?.clone();
        let nv = out.data.len();
        for k in 0..nv {
            let mut n = 1i32;
            for s in 1..ns {
                if mask.data[k] > 0 {
                    out.data[k] += tmp.data[s * nv + k];
                    n += 1;
                }
            }
            out.data[k] = (f64::from(out.data[k]) / f64::from(n)) as f32;
        }
        out.copy_properties(hdr);
        self.save(&out, fname)?;
        self.con
            .trace(self.tp, "Leaving TopupCF::WriteMeanUnwarped");
        Ok(())
    }

    /// `WriteJacobiansForDebug(fname)`.
    fn write_jacobians_debug(&mut self, fname: &str) -> Result<(), FslError> {
        self.con
            .trace(self.tp, "Entering TopupCF::WriteJacobiansForDebug");
        let sz = self.sm.image_size(self.con, SizeType::Target)?;
        let ns = self.sm.n_scans();
        let mut out = Volume::new(
            i64::from(sz[0]),
            i64::from(sz[1]),
            i64::from(sz[2]),
            ns as i64,
        );
        for i in 0..ns {
            let j = self.sm.get_jacobian(self.con, i, &mut self.field)?;
            out.set_sub_volume(i as i64, &j)?;
        }
        self.save(&out, fname)?;
        self.con
            .trace(self.tp, "Leaving TopupCF::WriteJacobiansForDebug");
        Ok(())
    }

    /// `WriteJacobians(fname)`: `fname_01`, `fname_02`, ...
    pub fn write_jacobians(&mut self, fname: &str) -> Result<(), FslError> {
        self.con.trace(self.tp, "Entering TopupCF::WriteJacobians");
        for i in 0..self.sm.n_scans() {
            let out = self.sm.get_jacobian(self.con, i, &mut self.field)?;
            self.save(&out, &format!("{fname}_{:02}", i + 1))?;
        }
        self.con.trace(self.tp, "Leaving TopupCF::WriteJacobians");
        Ok(())
    }

    /// `WriteField(fname)` (`--debug`): the field in the first corrected scan's image.
    fn write_field_debug(&mut self, fname: &str) -> Result<(), FslError> {
        self.con.trace(self.tp, "Entering TopupCF::WriteField");
        let rf = self.sm.get_scan(self.con, 0, &mut self.field)?;
        let out = fileio::field_volume(&rf, &mut self.field, 1.0, self.con)?;
        self.save(&out, fname)?;
        self.con.trace(self.tp, "Leaving TopupCF::WriteField");
        Ok(())
    }

    /// `WriteField(fname, hdr, sf)`: the field (Hz, times `sf`) in `hdr`'s first volume.
    pub fn write_field(&mut self, fname: &str, hdr: &Volume, sf: f64) -> Result<(), FslError> {
        self.con.trace(self.tp, "Entering TopupCF::WriteField");
        let rf = hdr.sub_volume(0)?;
        let out = fileio::field_volume(&rf, &mut self.field, sf, self.con)?;
        self.save(&out, fname)?;
        self.con.trace(self.tp, "Leaving TopupCF::WriteField");
        Ok(())
    }

    /// `WriteDisplacementFields(fname)`: `fname_01`, ... (4D: x, y, z in mm).
    pub fn write_displacement_fields(&mut self, fname: &str) -> Result<(), FslError> {
        self.con
            .trace(self.tp, "Entering TopupCF::WriteDisplacementFields");
        for i in 0..self.sm.n_scans() {
            let out = self
                .sm
                .get_displacement_field(self.con, i, &mut self.field)?;
            self.save(&out, &format!("{fname}_{:02}", i + 1))?;
        }
        self.con
            .trace(self.tp, "Leaving TopupCF::WriteDisplacementFields");
        Ok(())
    }

    /// `WriteMask(fname)` (`--debug` 2).
    fn write_mask(&mut self, fname: &str) -> Result<(), FslError> {
        self.con.trace(self.tp, "Entering TopupCF::WriteMask");
        let mask = self.sm.mask(self.con, &mut self.field)?.clone();
        self.save(&mask, fname)?;
        self.con.trace(self.tp, "Leaving TopupCF::WriteMask");
        Ok(())
    }

    /// `WriteMaskedDiff(fname)` (`--debug` 2): the mean minus each scan in the mask, in an
    /// image of the subsampled scans' size (the target grid's corner of it).
    fn write_masked_diff(&mut self, fname: &str) -> Result<(), FslError> {
        self.con.trace(self.tp, "Entering TopupCF::WriteMaskedDiff");
        let sz = self.sm.image_size(self.con, SizeType::Subsampled)?;
        let ns = self.sm.n_scans();
        let (sx, sy, sz3) = (i64::from(sz[0]), i64::from(sz[1]), i64::from(sz[2]));
        let mut out = Volume::new(sx, sy, sz3, ns as i64);
        let (mean, _, _, _, mask) = self.sm.means(self.con, &mut self.field, 2)?;
        let n = (sx * sy * sz3) as usize;
        for s in 0..ns {
            out.data[s * n..(s + 1) * n].fill(0.0);
            let scan = self.sm.get_scan(self.con, s, &mut self.field)?;
            for k in 0..mean.zsize() {
                for j in 0..mean.ysize() {
                    for i in 0..mean.xsize() {
                        if mask.value(i, j, k) != 0 {
                            let w = s * n + ((k * sy + j) * sx + i) as usize;
                            out.data[w] = mean.value(i, j, k) - scan.value(i, j, k);
                        }
                    }
                }
            }
        }
        self.save(&out, fname)?;
        self.con.trace(self.tp, "Leaving TopupCF::WriteMaskedDiff");
        Ok(())
    }

    /// `WriteMovementParameters(fname)`: one row of six per scan.
    pub fn write_movement_parameters(&self, fname: &str) -> Result<(), FslError> {
        self.con
            .trace(self.tp, "Entering TopupCF::WriteMovementParameters");
        let mp = self.sm.all_movement_parameters(self.con);
        let ns = self.sm.n_scans();
        let mut out = Mat::zeros(ns, 6);
        for s in 0..ns {
            for p in 0..6 {
                out.set(s, p, mp[s * 6 + p]);
            }
        }
        if let Err(printed) = write_ascii_matrix(fname, &out) {
            if let Some(l) = printed {
                self.con.err_line(&l);
            }
            return Err(fileio::fileio_error(&format!(
                "TopupFileWriter::TopupFileWriter: Failed to write movement parameter file {fname}"
            )));
        }
        self.con
            .trace(self.tp, "Leaving TopupCF::WriteMovementParameters");
        Ok(())
    }

    /// `WriteRigidBodyMatrices(fname)`: `fname_01.mat`, ... (a file that cannot be written is
    /// reported and skipped).
    pub fn write_rigid_body_matrices(&self, fname: &str) -> Result<(), FslError> {
        self.con
            .trace(self.tp, "Entering TopupCF::WriteRigidBodyMatrices");
        for s in 0..self.sm.n_scans() {
            let m = self.sm.rigid_body_matrix(self.con, s)?;
            if let Err(Some(l)) = write_ascii_matrix(&format!("{fname}_{:02}.mat", s + 1), &m) {
                self.con.err_line(&l);
            }
        }
        self.con
            .trace(self.tp, "Leaving TopupCF::WriteRigidBodyMatrices");
        Ok(())
    }

    // -------------------------------------------------------------------------------------
    // The regularisation

    /// `FieldEnergy()`.
    fn field_energy(&self) -> Result<f64, FslError> {
        self.con.trace(self.tp, "Entering TopupCF::FieldEnergy");
        match self.rt {
            Regularisation::BendingEnergy => self.field.bend_energy(1),
            Regularisation::MembraneEnergy => self.field.mem_energy(1),
        }
    }

    /// `FieldEnergyGrad()`.
    fn field_energy_grad(&self) -> Result<Vec<f64>, FslError> {
        self.con.trace(self.tp, "Entering TopupCF::FieldEnergyGrad");
        let g = match self.rt {
            Regularisation::BendingEnergy => self.field.bend_energy_grad()?,
            Regularisation::MembraneEnergy => self.field.mem_energy_grad()?,
        };
        self.con.trace(self.tp, "Leaving TopupCF::FieldEnergyGrad");
        Ok(g)
    }

    /// `FieldEnergyHess()`.
    fn field_energy_hess(&self) -> Result<BFMatrix, FslError> {
        self.con.trace(self.tp, "Entering TopupCF::FieldEnergyHess");
        let c = match self.rt {
            Regularisation::BendingEnergy => self.field.bend_energy_hess()?,
            // FSL-117: FSL's Hessian measures the derivatives in mm, its cost in voxels
            Regularisation::MembraneEnergy if self.mode.reproduce() => {
                self.con.notice(
                    &defects::FSL_117,
                    "Levenberg-Marquardt's membrane-energy Hessian in mm, its cost in voxels",
                );
                self.field.mem_energy_hess()?
            }
            Regularisation::MembraneEnergy => {
                self.con.notice(
                    &defects::FSL_117,
                    "Levenberg-Marquardt's membrane-energy Hessian in the cost's units",
                );
                self.field.mem_energy_hess_in_voxels()?
            }
        };
        let h = BFMatrix::sparse_from_csc(&c, self.hp, SPMAT_THREADS)?;
        self.con.trace(self.tp, "Leaving TopupCF::FieldEnergyHess");
        Ok(h)
    }

    // -------------------------------------------------------------------------------------
    // The parameters

    /// `set_field_params(p)`: the coefficients replaced when one differs by more than 1e-6.
    fn set_field_params(&mut self, p: &[f64]) -> Result<(), FslError> {
        self.con
            .trace(self.tp, "Entering TopupCF::set_field_params");
        if p.len() != self.npar() {
            return Err(topup_error(
                "Topup_CF::set_field_params: wrong size parameter vector p",
            ));
        }
        let nc = self.ndefpar();
        let changed = (0..nc).any(|i| (p[i] - self.field.coefs()[i]).abs() > 1e-6);
        if changed {
            self.field.set_coef(&p[..nc])?;
            self.sm.field_updated(self.con);
        }
        self.con.trace(self.tp, "Leaving TopupCF::set_field_params");
        Ok(())
    }

    /// `set_movement_params(p)`.
    fn set_movement_params(&mut self, p: &[f64]) -> Result<(), FslError> {
        self.con
            .trace(self.tp, "Entering TopupCF::set_movement_params");
        if !self.mf {
            if p.len() != self.npar() {
                return Err(topup_error(
                    "Topup_CF::set_movement_params: wrong size parameter vector p",
                ));
            }
            let nc = self.ndefpar();
            self.sm.set_movement_parameters(self.con, &p[nc..])?;
        }
        self.con
            .trace(self.tp, "Leaving TopupCF::set_movement_params");
        Ok(())
    }

    /// `sum_of_prod(i1, i2, mask)`: `Σ i1·i2` (each product in `float`) in the mask.
    fn sum_of_prod(&self, i1: &Volume, i2: &Volume, mask: &Volume<i8>) -> f64 {
        self.con.trace(self.tp, "Entering TopupCF::sum_of_prod");
        let mut sum = 0.0f64;
        for (k, &m) in mask.data.iter().enumerate().take(i1.data.len()) {
            if m != 0 {
                sum += f64::from(i1.data[k] * i2.data[k]);
            }
        }
        self.con.trace(self.tp, "Leaving TopupCF::sum_of_prod");
        sum
    }

    /// `movement_hessian(s1, s2, mask, field)`: the products of the movement derivatives of
    /// scans `s1` and `s2`.
    fn movement_hessian(
        &mut self,
        s1: usize,
        s2: usize,
        mask: &Volume<i8>,
    ) -> Result<Mat, FslError> {
        self.con
            .trace(self.tp, "Entering TopupCF::movement_hessian");
        let n1 = self.sm.n_movement_parameters_for(s1)?;
        let n2 = self.sm.n_movement_parameters_for(s2)?;
        let mut rval = Mat::zeros(n1, n2);
        let mut d1 = Vec::with_capacity(n1);
        for d in 0..n1 {
            d1.push(
                self.sm
                    .get_movement_derivative(self.con, s1, d, &mut self.field)?,
            );
        }
        if s1 == s2 {
            for a in 0..n1 {
                for b in a..n1 {
                    let v = self.sum_of_prod(&d1[a], &d1[b], mask);
                    rval.set(a, b, v);
                    if a != b {
                        rval.set(b, a, v);
                    }
                }
            }
        } else {
            let mut d2 = Vec::with_capacity(n2);
            for d in 0..n2 {
                d2.push(
                    self.sm
                        .get_movement_derivative(self.con, s2, d, &mut self.field)?,
                );
            }
            for a in 0..n1 {
                for b in 0..n2 {
                    rval.set(a, b, self.sum_of_prod(&d1[a], &d2[b], mask));
                }
            }
        }
        self.con.trace(self.tp, "Leaving TopupCF::movement_hessian");
        Ok(rval)
    }

    /// The sizes `n` (voxels in the mask) and `m` (scans) of the cost's normalisation.
    fn nm(mask: &Volume<i8>, m: usize) -> (u32, u32) {
        (vol::sum(mask) as u32, m as u32)
    }
}

/// `save_volume`: an `FSLOUTPUTTYPE` FSL cannot write ends the program (`exit(EXIT_FAILURE)`).
pub fn save<T: Elem>(
    v: &Volume<T>,
    name: &str,
    fsl: &Settings,
    mode: UpstreamDefects,
) -> Result<(), FslError> {
    let opts =
        SaveOptions::new(volume::BUILDSTRING.as_bytes()).reproducing_offset(mode.reproduce());
    match v.save_volume(name, fsl, &opts) {
        Ok(_) => Ok(()),
        Err(SaveError::OutputType(o)) => std::panic::resume_unwind(Box::new(ExitNow {
            code: 1,
            printed: o.printed,
        })),
        Err(SaveError::Exception(e)) => Err(e),
    }
}

impl NonlinCf for TopupCf<'_, '_> {
    /// `cf(p)`.
    fn cf(&mut self, p: &[f64]) -> Result<f64, FslError> {
        self.con.trace(self.tp, "Entering TopupCF::cf");
        if p.len() != self.npar() {
            return Err(topup_error("Topup_CF::cf: wrong size parameter vector p"));
        }
        self.set_field_params(p)?;
        self.set_movement_params(p)?;
        let (mean, _, _, _, mask) = self.sm.means(self.con, &mut self.field, 2)?;
        let mut ssd = 0.0f64;
        let (n, m) = Self::nm(&mask, self.sm.n_scans());
        for s in 0..self.sm.n_scans() {
            let scan = self.sm.get_scan(self.con, s, &mut self.field)?;
            for (k, &mk) in mask.data.iter().enumerate() {
                if mk != 0 {
                    let d = mean.data[k] - scan.data[k];
                    ssd += f64::from(d * d);
                }
            }
        }
        ssd /= f64::from(n.wrapping_mul(m.wrapping_sub(1)));
        self.lssd = ssd;
        if self.dl > 0 {
            self.attempt += 1;
            let ds = self.debug_string();
            self.write_unwarped_debug(&format!("TopupDebugUnwarped{ds}"))?;
            self.write_jacobians_debug(&format!("TopupDebugJacobians{ds}"))?;
            self.write_field_debug(&format!("TopupDebugField{ds}"))?;
            self.write_movement_parameters(&format!("TopupDebugMovementParameters{ds}.txt"))?;
            if self.dl > 1 {
                self.write_mask(&format!("TopupDebugMask{ds}"))?;
                self.write_masked_diff(&format!("TopupDebugMaskedDiff{ds}"))?;
            }
        }
        let g = |v: f64| larmorx_nc_core::cnum::fmt_g(v, 6);
        if self.verb {
            self.con.put(&format!("SSD = {}\tn = {n}", g(ssd)));
        }
        let reg = self.lambda_now() * self.field_energy()?;
        if self.verb {
            self.con.put(&format!("\tReg = {}", g(reg)));
        }
        let cost = ssd + reg;
        if self.verb {
            self.con.line(&format!("\tCost = {}", g(cost)));
        }
        self.con.trace(self.tp, "Leaving TopupCF::cf");
        Ok(cost)
    }

    /// `grad(p)`.
    fn grad(&mut self, p: &[f64]) -> Result<Vec<f64>, FslError> {
        self.con.trace(self.tp, "Entering TopupCF::grad");
        if p.len() != self.npar() {
            return Err(topup_error("Topup_CF::grad: wrong size parameter vector p"));
        }
        self.set_field_params(p)?;
        if !self.mf {
            self.set_movement_params(p)?;
        }
        let npar = self.npar();
        let nd = self.ndefpar();
        let mut gradient = vec![0.0f64; npar];
        let (mean, mean_alpha, mean_beta, mean_gamma, mask) =
            self.sm.means(self.con, &mut self.field, 5)?;
        let (n, m) = Self::nm(&mask, self.sm.n_scans());
        let zeros = || {
            let mut v = vol::grid(
                mean.xsize(),
                mean.ysize(),
                mean.zsize(),
                [mean.xdim(), mean.ydim(), mean.zdim()],
            );
            v.copy_properties(&mean);
            v
        };
        let (hb, hg) = (self.sm.has_beta(), self.sm.has_gamma());
        let mut abf0 = zeros();
        let mut abf1 = if hb { Some(zeros()) } else { None };
        let mut abf2 = if hg { Some(zeros()) } else { None };
        for i in 0..self.sm.n_scans() {
            let diff = vol::sub(&mean, &self.sm.get_scan(self.con, i, &mut self.field)?)?;
            let a = vol::sub(
                &mean_alpha,
                &self.sm.get_alpha(self.con, i, &mut self.field)?,
            )?;
            vol::add_assign(&mut abf0, &vol::mul(&a, &diff)?)?;
            if self.sm.has_beta_of(self.con, i)? {
                let b = vol::sub(&mean_beta, &self.sm.get_beta(self.con, i, &mut self.field)?)?;
                vol::add_assign(abf1.as_mut().expect("beta"), &vol::mul(&b, &diff)?)?;
            }
            if self.sm.has_gamma_of(self.con, i)? {
                let c = vol::sub(
                    &mean_gamma,
                    &self.sm.get_gamma(self.con, i, &mut self.field)?,
                )?;
                vol::add_assign(abf2.as_mut().expect("gamma"), &vol::mul(&c, &diff)?)?;
            }
        }
        let den = f64::from(n.wrapping_mul(m.wrapping_sub(1)));
        let mut add = |jte: Vec<f64>| {
            for (g, j) in gradient[..nd].iter_mut().zip(jte) {
                *g += (2.0 * j) / den;
            }
        };
        add(self.field.jte_field(&abf0, Some(&mask))?);
        if let Some(b) = &abf1 {
            add(self.field.jte(&[1, 0, 0], b, Some(&mask))?);
        }
        if let Some(c) = &abf2 {
            add(self.field.jte(&[0, 1, 0], c, Some(&mask))?);
        }
        if !self.mf {
            let mut gi = nd;
            for s in 0..self.sm.n_scans() {
                let diff = vol::sub(&mean, &self.sm.get_scan(self.con, s, &mut self.field)?)?;
                for d in 0..self.sm.n_movement_parameters_for(s)? {
                    let dv = self
                        .sm
                        .get_movement_derivative(self.con, s, d, &mut self.field)?;
                    gradient[gi] = -(2.0 / den) * self.sum_of_prod(&diff, &dv, &mask);
                    gi += 1;
                }
            }
        }
        if self.dl > 0 {
            self.iter += 1;
            self.attempt = 0;
            let name = format!("TopupDebugGradient_{}.txt", self.debug_string());
            self.write_vector(&name, &gradient);
        }
        let lam = self.lambda_now();
        let eg = self.field_energy_grad()?;
        for (g, e) in gradient[..nd].iter_mut().zip(eg) {
            *g += lam * e;
        }
        if self.dl > 1 {
            let name = format!("TopupDebugGradientWithReg_{}.txt", self.debug_string());
            self.write_vector(&name, &gradient);
        }
        self.con.trace(self.tp, "Leaving TopupCF::grad");
        Ok(gradient)
    }

    /// `hess(p, iptr)` (`iptr` is not used).
    fn hess(&mut self, p: &[f64], _iptr: Option<BFMatrix>) -> Result<BFMatrix, FslError> {
        self.con.trace(self.tp, "Entering TopupCF::hess");
        if p.len() != self.npar() {
            return Err(topup_error("Topup_CF::grad: wrong size parameter vector p"));
        }
        self.set_field_params(p)?;
        if !self.mf {
            self.set_movement_params(p)?;
        }
        let (mean, mean_alpha, mean_beta, mean_gamma, mask) =
            self.sm.means(self.con, &mut self.field, 5)?;
        let (n, m) = Self::nm(&mask, self.sm.n_scans());
        let zeros = || {
            let mut v = vol::grid(
                mean.xsize(),
                mean.ysize(),
                mean.zsize(),
                [mean.xdim(), mean.ydim(), mean.zdim()],
            );
            v.copy_properties(&mean);
            v
        };
        let (hb, hg) = (self.sm.has_beta(), self.sm.has_gamma());
        let mut abc: Vec<Option<Volume>> = vec![None; 6];
        abc[0] = Some(zeros());
        if hb {
            abc[1] = Some(zeros());
            abc[3] = Some(zeros());
        }
        if hg {
            abc[2] = Some(zeros());
            abc[5] = Some(zeros());
        }
        if hb && hg {
            abc[4] = Some(zeros());
        }
        let acc = |abc: &mut Vec<Option<Volume>>, k: usize, a: &Volume, b: &Volume| {
            vol::add_assign(abc[k].as_mut().expect("allocated"), &vol::mul(a, b)?)
        };
        for i in 0..self.sm.n_scans() {
            let ad = vol::sub(
                &mean_alpha,
                &self.sm.get_alpha(self.con, i, &mut self.field)?,
            )?;
            acc(&mut abc, 0, &ad, &ad)?;
            if hb {
                let bd = vol::sub(&mean_beta, &self.sm.get_beta(self.con, i, &mut self.field)?)?;
                acc(&mut abc, 1, &ad, &bd)?;
                acc(&mut abc, 3, &bd, &bd)?;
                if hg {
                    let gd = vol::sub(
                        &mean_gamma,
                        &self.sm.get_gamma(self.con, i, &mut self.field)?,
                    )?;
                    acc(&mut abc, 2, &ad, &gd)?;
                    acc(&mut abc, 4, &bd, &gd)?;
                    acc(&mut abc, 5, &gd, &gd)?;
                }
            } else if hg {
                let gd = vol::sub(
                    &mean_gamma,
                    &self.sm.get_gamma(self.con, i, &mut self.field)?,
                )?;
                acc(&mut abc, 2, &ad, &gd)?;
                acc(&mut abc, 5, &gd, &gd)?;
            }
        }
        let mut ones = zeros();
        vol::fill(&mut ones, 1.0);
        let hp = self.hp;
        let sp =
            |c: &larmorx_nc_core::basisfield::Csc| BFMatrix::sparse_from_csc(c, hp, SPMAT_THREADS);
        let a = |k: usize| abc[k].as_ref().expect("allocated");
        let mut nonlin_bit = sp(&self.field.jtj(&[0, 0, 0], a(0), &ones, Some(&mask))?)?;
        let d0 = [0u32, 0, 0];
        let dx = [1u32, 0, 0];
        let dy = [0u32, 1, 0];
        if hb {
            let tmp = sp(&self.field.jtj_cross(&d0, a(1), &dx, &ones, Some(&mask))?)?;
            nonlin_bit.add_to_me(&tmp, 1.0, SPMAT_THREADS)?;
            nonlin_bit.add_to_me(&tmp.transpose(SPMAT_THREADS), 1.0, SPMAT_THREADS)?;
            let tmp = sp(&self.field.jtj(&dx, a(3), &ones, Some(&mask))?)?;
            nonlin_bit.add_to_me(&tmp, 1.0, SPMAT_THREADS)?;
        }
        if hg {
            let tmp = sp(&self.field.jtj_cross(&d0, a(2), &dy, &ones, Some(&mask))?)?;
            nonlin_bit.add_to_me(&tmp, 1.0, SPMAT_THREADS)?;
            nonlin_bit.add_to_me(&tmp.transpose(SPMAT_THREADS), 1.0, SPMAT_THREADS)?;
            let tmp = sp(&self.field.jtj(&dy, a(5), &ones, Some(&mask))?)?;
            nonlin_bit.add_to_me(&tmp, 1.0, SPMAT_THREADS)?;
        }
        if hb && hg {
            let tmp = sp(&self.field.jtj_cross(&dx, a(4), &dy, &ones, Some(&mask))?)?;
            nonlin_bit.add_to_me(&tmp, 1.0, SPMAT_THREADS)?;
            nonlin_bit.add_to_me(&tmp.transpose(SPMAT_THREADS), 1.0, SPMAT_THREADS)?;
        }
        let den = f64::from(n.wrapping_mul(m.wrapping_sub(1)));
        nonlin_bit.mul_me_by_scalar(2.0 / den);
        let lam = self.lambda_now();
        if lam != 0.0 {
            let eh = self.field_energy_hess()?;
            nonlin_bit.add_to_me(&eh, lam, SPMAT_THREADS)?;
        }
        if self.dl > 1 {
            let mut out = Volume::new(mean.xsize(), mean.ysize(), mean.zsize(), 6);
            out.copy_properties(&mean);
            for (k, v) in abc.iter().enumerate() {
                if let Some(v) = v {
                    out.set_sub_volume(k as i64, v)?;
                }
            }
            self.save(&out, &format!("TopupDebugABC{}", self.debug_string()))?;
        }
        drop(abc);
        if !self.mf {
            let ns = self.sm.n_scans();
            let mut tiles = vec![0usize; ns];
            for (s, t) in tiles.iter_mut().enumerate().skip(1) {
                *t = self.sm.n_movement_parameters_for(s)?;
            }
            let off: Vec<usize> = tiles
                .iter()
                .scan(0, |acc, &t| {
                    let o = *acc;
                    *acc += t;
                    Some(o)
                })
                .collect();
            let tot: usize = tiles.iter().sum();
            let mut tiled = Mat::zeros(tot, tot);
            let mm = f64::from(m);
            for s1 in 1..ns {
                for s2 in s1..ns {
                    let mh = self.movement_hessian(s1, s2, &mask)?;
                    if s1 == s2 {
                        let k = (2.0 / den) * (1.0 - 1.0 / mm);
                        tiled.set_sub(off[s1], off[s2], &mh.scale(k));
                    } else {
                        let k = -((1.0 / den) * (2.0 / mm));
                        let t = mh.scale(k);
                        tiled.set_sub(off[s1], off[s2], &t);
                        tiled.set_sub(off[s2], off[s1], &t.t());
                    }
                }
            }
            let nd = self.ndefpar();
            let nmv = self.nmovpar();
            let mut interaction = Mat::zeros(nd, nmv);
            let mut offset = 0usize;
            for s in 1..ns {
                let ad = vol::sub(
                    &mean_alpha,
                    &self.sm.get_alpha(self.con, s, &mut self.field)?,
                )?;
                let bd = if hb {
                    Some(vol::sub(
                        &mean_beta,
                        &self.sm.get_beta(self.con, s, &mut self.field)?,
                    )?)
                } else {
                    None
                };
                let gd = if hg {
                    Some(vol::sub(
                        &mean_gamma,
                        &self.sm.get_gamma(self.con, s, &mut self.field)?,
                    )?)
                } else {
                    None
                };
                let nps = self.sm.n_movement_parameters_for(s)?;
                for d in 0..nps {
                    let dm = self
                        .sm
                        .get_movement_derivative(self.con, s, d, &mut self.field)?;
                    let mut col: Vec<f64> = self
                        .field
                        .jte2_field(&dm, &ad, Some(&mask))?
                        .iter()
                        .map(|v| -v)
                        .collect();
                    if let Some(b) = &bd {
                        let t = self.field.jte2(&dx, &dm, b, Some(&mask))?;
                        col.iter_mut().zip(t).for_each(|(c, t)| *c -= t);
                    }
                    if let Some(g) = &gd {
                        let t = self.field.jte2(&dy, &dm, g, Some(&mask))?;
                        col.iter_mut().zip(t).for_each(|(c, t)| *c -= t);
                    }
                    for (r, v) in col.into_iter().enumerate() {
                        interaction.set(r, offset + d, v);
                    }
                }
                offset += nps;
            }
            let k = 2.0 / den;
            for v in interaction.as_mut_slice() {
                *v *= k;
            }
            let move_bit = BFMatrix::Full(tiled);
            let mut bf_interaction = BFMatrix::Full(interaction);
            nonlin_bit.vert_concat_below_me(&bf_interaction.transpose(1), SPMAT_THREADS)?;
            bf_interaction.vert_concat_below_me(&move_bit, SPMAT_THREADS)?;
            nonlin_bit.hor_concat2_my_right(&bf_interaction, SPMAT_THREADS)?;
        }
        if self.dl > 2
            && let Some(text) = nonlin_bit.print_text()
        {
            let name = format!("TopupDebugHess{}.txt", self.debug_string());
            let _ = std::fs::write(name, text);
        }
        self.con.trace(self.tp, "Leaving TopupCF::hess");
        Ok(nonlin_bit)
    }
}

impl TopupCf<'_, '_> {
    /// `write_ascii_matrix(name, vector)` (its failure reported, not fatal).
    fn write_vector(&self, name: &str, v: &[f64]) {
        let m = Mat::col(v);
        if let Err(Some(l)) = write_ascii_matrix(name, &m) {
            self.con.err_line(&l);
        }
    }
}
