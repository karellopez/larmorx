// SPDX-License-Identifier: LicenseRef-FSL
// Translated to Rust from FSL 6.0.7.23: fsl-newimage 2601.0 (costfns.{h,cc}: the Costfn class as
// FLIRT holds it: its constructors, set_no_bins, cost and the weighted cost, and the member
// wrappers). Copyright (C) 1999-2023 University of Oxford (Mark Jenkinson). Rust translation
// Copyright 2026 Karel Lopez Vilaret; modified from the original. Distributed under the FSL
// Licence (see LICENSE).
//! FLIRT's image pair (`NEWIMAGE::Costfn` as `flirt.cc` uses it). FSL's `Costfn` holds
//! *references* to the reference, the test volume and the two weight volumes, not copies: FLIRT
//! later changes the volumes those references name (each `setscale` refreshes the test volume,
//! a scale it has no volume for replaces the 1 mm reference), and a pair built earlier then sees
//! the new contents. The pair here names the volumes it refers to ([`RefSlot`]); the run resolves
//! them at each evaluation, so that the same happens. What `Costfn` owns (the reference's bins,
//! the `-p log p` table, the test volume's centre of mass, BBR's boundary points and settings,
//! the evaluation count) it owns here too.

use larmorx_nc_core::arma::Mat;
use larmorx_nc_core::bbr::{self, BbrMessages, BbrPoints, BbrSettings};
use larmorx_nc_core::costfns::{self, CostType, Weights};
use larmorx_nc_core::volume::Volume;

/// The reference volume a pair refers to: one of `main`'s `refvol`, `refvol_2`, `refvol_4`,
/// `refvol_8`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RefSlot {
    Ref1,
    Ref2,
    Ref4,
    Ref8,
}

/// What one evaluation printed on standard error (in FSL's order).
#[derive(Clone, Debug, Default)]
pub struct CostMessages {
    pub lines: Vec<String>,
    /// glibc's abort after FSL wrote before its arrays (a correlation ratio with no bins, FSL-21).
    pub heap_corrupted: bool,
}

/// `Costfn`.
#[derive(Clone, Debug)]
pub struct Pair {
    pub ref_slot: RefSlot,
    /// `validweights`: built with the weight volumes (`-refweight`, `-inweight`).
    pub validweights: bool,
    /// `testCog`: the test volume's centre of mass when the pair was built (scaled mm).
    pub test_cog: [f64; 3],
    pub costtype: CostType,
    pub smoothsize: f32,
    pub fuzzyfrac: f32,
    pub no_bins: i32,
    pub bindex: Vec<i32>,
    pub plnp: Vec<f64>,
    /// `p_count`.
    pub count: u64,
    pub bbr_points: BbrPoints,
    pub bbr: BbrSettings,
    /// FSL-14 corrected: points whose values are not finite are left out of the cost.
    pub skip_nonfinite: bool,
    /// `set_debug_mode(true)` with the bbr cost: the debug volume each evaluation writes.
    pub bbr_debug: Option<bbr::BbrDebug>,
}

impl Pair {
    /// `Costfn(refv, inputv)` or, with `validweights`, `Costfn(refv, inputv, refweight,
    /// inweight)`; `test_cog` is `inputv.cog("scaled_mm")`.
    pub fn new(ref_slot: RefSlot, validweights: bool, test_cog: [f64; 3]) -> Pair {
        Pair {
            ref_slot,
            validweights,
            test_cog,
            costtype: CostType::CorrRatio,
            smoothsize: 1.0,
            fuzzyfrac: 0.5,
            no_bins: 0,
            bindex: Vec::new(),
            plnp: Vec::new(),
            count: 0,
            bbr_points: BbrPoints::default(),
            bbr: BbrSettings::default(),
            skip_nonfinite: false,
            bbr_debug: None,
        }
    }

    /// `is_bbr_set()`: boundary points exist.
    pub fn is_bbr_set(&self) -> bool {
        !self.bbr_points.is_empty()
    }

    /// `set_no_bins(n)` on the pair's reference: the bins and the `-p log p` table. `None` when
    /// FSL divides by zero (`no_bins = -1`: signal 8) or allocates a negative length (`no_bins
    /// <= -2`: `std::bad_array_new_length`).
    pub fn set_no_bins(&mut self, refvol: &Volume, n_bins: i32) -> Result<(), BinsFailure> {
        if n_bins <= -2 {
            return Err(BinsFailure::NegativeLength);
        }
        let Some(plnp) = costfns::plnp_table(refvol.nvoxels() as u64, n_bins) else {
            return Err(BinsFailure::DivisionByZero);
        };
        self.no_bins = n_bins;
        self.plnp = plnp;
        self.bindex = costfns::bin_index(refvol, n_bins).0;
        Ok(())
    }

    /// `cost(affmat)` (and, for BBR with a field map, `cost(affmat, nonlin_params)`): the cost of
    /// `aff` (FLIRT millimetres to millimetres) between the volumes the pair refers to.
    /// `weights` are `(rweight, tweight)`: FLIRT's `global_refweight` and `global_testweight`.
    pub fn cost(
        &mut self,
        refvol: &Volume,
        testvol: &Volume,
        weights: (&Volume, &Volume),
        aff: &Mat,
        nonlin: Option<f64>,
        msgs: &mut CostMessages,
    ) -> f32 {
        let skip = self.skip_nonfinite;
        if let Some(nl) = nonlin {
            // cost(affmat, nonlin_params): BBR only; the weights are bbr's own test.
            if self.costtype == CostType::Bbr {
                return self.bbr_cost(refvol, testvol, weights, aff, nl, msgs);
            }
            msgs.lines.push("Invalid cost function type".to_owned());
            return 0.0;
        }
        if self.validweights {
            return self.weighted(refvol, testvol, weights, aff, msgs, skip);
        }
        let ss = self.smoothsize;
        match self.costtype {
            CostType::NormCorr => {
                self.count += 1;
                let c = if ss > 0.0 {
                    costfns::p_normcorr_weighted(refvol, testvol, None, aff, ss, false, skip)
                } else {
                    costfns::p_normcorr(refvol, testvol, aff, skip)
                };
                (1.0 - f64::from(c.abs())) as f32
            }
            CostType::NormCorrSinc => {
                self.count += 1;
                let c = costfns::p_normcorr_weighted(refvol, testvol, None, aff, ss, true, skip);
                (1.0 - f64::from(c.abs())) as f32
            }
            CostType::LeastSq => {
                self.count += 1;
                if ss > 0.0 {
                    costfns::p_leastsquares_weighted(refvol, testvol, None, aff, ss, skip)
                } else {
                    costfns::p_leastsquares(refvol, testvol, aff, skip)
                }
            }
            CostType::LabelDiff => {
                self.count += 1;
                if ss > 0.0 {
                    costfns::p_labeldiff_weighted(refvol, testvol, None, aff, ss)
                } else {
                    costfns::p_labeldiff(refvol, testvol, aff)
                }
            }
            CostType::Bbr => self.bbr_cost(refvol, testvol, weights, aff, 1.0, msgs),
            CostType::CorrRatio => {
                self.count += 1;
                let (c, corrupted) = if ss > 0.0 {
                    costfns::p_corr_ratio_weighted(
                        refvol,
                        testvol,
                        None,
                        &self.bindex,
                        aff,
                        self.no_bins,
                        ss,
                        skip,
                    )
                } else {
                    costfns::p_corr_ratio(refvol, testvol, &self.bindex, aff, self.no_bins, skip)
                };
                msgs.heap_corrupted |= corrupted;
                (1.0 - f64::from(c)) as f32
            }
            CostType::Woods => {
                self.count += 1;
                costfns::p_woods_fn(refvol, testvol, &self.bindex, aff, self.no_bins, skip)
            }
            CostType::MutualInfo | CostType::NormMI => {
                self.count += 1;
                let (mx, mn) = testvol.extrema();
                let (je, m1, m2) = if ss > 0.0 || self.fuzzyfrac > 0.0 {
                    costfns::calc_weighted_entropy(
                        refvol,
                        testvol,
                        None,
                        &self.bindex,
                        aff,
                        mn,
                        mx,
                        self.no_bins,
                        ss,
                        self.fuzzyfrac,
                        skip,
                    )
                } else {
                    costfns::calc_entropy(
                        refvol,
                        testvol,
                        &self.bindex,
                        aff,
                        mn,
                        mx,
                        self.no_bins,
                        &self.plnp,
                    )
                };
                mi_value(self.costtype, je, m1, m2)
            }
        }
    }

    /// `cost(affmat, refweight, testweight)`: the fully weighted costs.
    fn weighted(
        &mut self,
        refvol: &Volume,
        testvol: &Volume,
        weights: (&Volume, &Volume),
        aff: &Mat,
        msgs: &mut CostMessages,
        skip: bool,
    ) -> f32 {
        let w = Some(Weights {
            refweight: weights.0,
            testweight: weights.1,
        });
        let ss = self.smoothsize;
        match self.costtype {
            CostType::NormCorr => {
                self.count += 1;
                let c = costfns::p_normcorr_weighted(refvol, testvol, w, aff, ss, false, skip);
                (1.0 - f64::from(c)) as f32
            }
            CostType::NormCorrSinc => {
                msgs.lines.push(
                    "WARNING: NormCorrSinc is not implemented with cost function weighting"
                        .to_owned(),
                );
                self.count += 1;
                let c = costfns::p_normcorr_weighted(refvol, testvol, None, aff, ss, true, skip);
                (1.0 - f64::from(c.abs())) as f32
            }
            CostType::LeastSq => {
                self.count += 1;
                costfns::p_leastsquares_weighted(refvol, testvol, w, aff, ss, skip)
            }
            CostType::LabelDiff => {
                self.count += 1;
                costfns::p_labeldiff_weighted(refvol, testvol, w, aff, ss)
            }
            CostType::Bbr => self.bbr_cost(refvol, testvol, weights, aff, 1.0, msgs),
            CostType::CorrRatio => {
                self.count += 1;
                let (c, corrupted) = costfns::p_corr_ratio_weighted(
                    refvol,
                    testvol,
                    w,
                    &self.bindex,
                    aff,
                    self.no_bins,
                    ss,
                    skip,
                );
                msgs.heap_corrupted |= corrupted;
                (1.0 - f64::from(c)) as f32
            }
            CostType::Woods => {
                msgs.lines.push(
                    "WARNING: Woods is not implemented with cost function weighting".to_owned(),
                );
                self.count += 1;
                costfns::p_woods_fn(refvol, testvol, &self.bindex, aff, self.no_bins, skip)
            }
            CostType::MutualInfo | CostType::NormMI => {
                self.count += 1;
                let (mx, mn) = testvol.extrema();
                let (je, m1, m2) = costfns::calc_weighted_entropy(
                    refvol,
                    testvol,
                    w,
                    &self.bindex,
                    aff,
                    mn,
                    mx,
                    self.no_bins,
                    ss,
                    self.fuzzyfrac,
                    skip,
                );
                mi_value(self.costtype, je, m1, m2)
            }
        }
    }

    /// `bbr(aff, nonlin_params)`, the weights used when the pair was built with them.
    fn bbr_cost(
        &mut self,
        refvol: &Volume,
        testvol: &Volume,
        weights: (&Volume, &Volume),
        aff: &Mat,
        nonlin: f64,
        msgs: &mut CostMessages,
    ) -> f32 {
        self.count += 1;
        let mut m = BbrMessages::default();
        let w = if self.validweights {
            Some(weights)
        } else {
            None
        };
        let v = bbr::bbr_cost(
            refvol,
            testvol,
            w,
            aff,
            nonlin,
            &self.bbr_points,
            &self.bbr,
            &mut m,
            self.bbr_debug.as_mut(),
        );
        for _ in 0..m.nan_pairs {
            msgs.lines
                .push("WARNING:: Found NaN in BBR cost".to_owned());
        }
        if m.nan_final {
            msgs.lines
                .push("WARNING:: Found NaN in BBR cost (2)".to_owned());
        }
        v
    }
}

/// `-mutual_info` / `-normalised_mutual_info` from the entropies (the normalised form 0 when the
/// joint entropy is about 0).
fn mi_value(costtype: CostType, je: f32, m1: f32, m2: f32) -> f32 {
    if costtype == CostType::MutualInfo {
        -(m1 + m2 - je)
    } else {
        let normmi = if f64::from(je.abs()) < 1e-9 {
            0.0
        } else {
            (m1 + m2) / je
        };
        -normmi
    }
}

/// How `set_no_bins` fails on a negative number of bins (FSL-66's crashes, as in mcflirt).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BinsFailure {
    /// `no_bins = -1`: an integer division by zero (signal 8).
    DivisionByZero,
    /// `no_bins <= -2`: `std::bad_array_new_length` (abort).
    NegativeLength,
}
