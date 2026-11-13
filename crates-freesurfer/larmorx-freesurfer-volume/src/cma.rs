// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! The CMA labels (`include/cma.h`: the label numbers, the `IS_*` tests) and the segmentation
//! functions of `utils/cma.cpp` that need only volumes (3.1-35 piece c): `IsSubCorticalGray`,
//! `MRIasegContraLatLabel`, `SupraTentorialVolCorrection`, `CorticalGMVolCorrection`,
//! `MRIlrswapAseg`, `MRIlrswapSeg`, `MRIfixAsegWithRibbon`, `MRIoneHotEncode`, and the cache of
//! the brain volume statistics (`CacheBrainVolumeStats`, `ReadCachedBrainVolumeStats`, its compute
//! step the caller's: `ComputeBrainVolumeStats` needs surfaces).
//!
//! The label numbers keep `cma.h`'s names (one constant per `#define`, transcribed mechanically,
//! in its order); the `IS_*` macros are generic functions over the label's type ([`Label`]: `int`,
//! `float` or `double`, compared as C compares the macro's argument with the integer constants).
//!
//! Defects (`larmorx_core::defects`; `docs/findings/freesurfer-volume.md` §8):
//! - **FS-311** (undefined conversion): a voxel value (or a label list's entry) that is NaN or
//!   outside the `int` range becomes the label -2147483648 (x86-64's `cvttss2si`). By default NaN
//!   is 0 and the rest saturates, noted; `reproduce` gives -2147483648.
//! - **FS-312** (wrong result): `SupraTentorialVolCorrection` and `CorticalGMVolCorrection` test
//!   `Left_Thalamus` and `Right_Thalamus` twice each (two names that were 9 and 10 now both 10, and
//!   49 twice): a thalamus voxel outside the pial surface, or inside the ribbon, counts twice. Once
//!   by default, noted; `reproduce` twice.
//! - **FS-313** (wrong result): `CorticalGMVolCorrection` subtracts `Left_vessel` voxels in the
//!   ribbon but not `Right_vessel` ones. Both by default, noted; `reproduce` the left only.
//! - **FS-314** (crash): `ReadCachedBrainVolumeStats` of a line that names a measure, has no number
//!   and ends in white space takes the last character off an empty word (`std::string::pop_back`):
//!   the program aborts. By default that line gives no number (the stat is then missing, as
//!   for a line without one); an error under `reproduce`.
//! - **FS-315** (memory): `MRIlrswapSeg` reads the label lists' first row as far as their row
//!   count: lists with fewer numbers than rows are read past their end. An error in both modes.
#![allow(non_upper_case_globals)]

use std::io::Write;
use std::path::Path;

use larmorx_core::defects::{FS_311, FS_312, FS_313, FS_314, FS_315};
use larmorx_freesurfer_core::cstd::fmt_f;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::{self, Fatal};
use larmorx_freesurfer_core::matrix::Matrix;
use larmorx_freesurfer_core::ret::Ret;
use larmorx_freesurfer_mri::copy::mri_copy;
use larmorx_freesurfer_mri::ctab::ctab_deep_copy;
use larmorx_freesurfer_mri::mri::{
    MRI_UCHAR, Mri, mri_alloc_sequence, mri_copy_header, mri_copy_pulse_parameters,
};
use larmorx_freesurfer_mri::voxel::{mri_get_vox_val, mri_set_vox_val};

use crate::arith::mri_clone;

/// A label as `cma.h`'s macros take it: `int`, `float` or `double`; the macros compare it with
/// the integer label numbers (every one below 32768) converted to its type.
pub trait Label: Copy + PartialEq + PartialOrd + From<i16> {}
impl<T: Copy + PartialEq + PartialOrd + From<i16>> Label for T {}

/// A label number as the type `T` (`cma.h`'s numbers are all below 32768).
fn k<T: Label>(v: i32) -> T {
    T::from(i16::try_from(v).expect("cma.h's label numbers fit in 16 bits"))
}

// --- generated from include/cma.h (every integer #define, in its order; a name defined twice once) ---
pub const Unknown: i32 = 0;
pub const Left_Cerebral_Exterior: i32 = 1;
pub const Left_Cerebral_White_Matter: i32 = 2;
pub const Left_Cerebral_Cortex: i32 = 3;
pub const Left_Lateral_Ventricle: i32 = 4;
pub const Left_Inf_Lat_Vent: i32 = 5;
pub const Left_Cerebellum_Exterior: i32 = 6;
pub const Left_Cerebellum_White_Matter: i32 = 7;
pub const Left_Cerebellum_Cortex: i32 = 8;
pub const Left_Thalamus_Proper: i32 = 10;
pub const Left_Thalamus: i32 = 10;
pub const Left_Caudate: i32 = 11;
pub const Left_Putamen: i32 = 12;
pub const Left_Pallidum: i32 = 13;
pub const Third_Ventricle: i32 = 14;
pub const Fourth_Ventricle: i32 = 15;
pub const Brain_Stem: i32 = 16;
pub const Left_Hippocampus: i32 = 17;
pub const Left_Amygdala: i32 = 18;
pub const Left_Insula: i32 = 19;
pub const Left_Operculum: i32 = 20;
pub const Line_1: i32 = 21;
pub const Line_2: i32 = 22;
pub const Line_3: i32 = 23;
pub const CSF: i32 = 24;
pub const Left_Lesion: i32 = 25;
pub const Left_Accumbens_area: i32 = 26;
pub const Left_Substancia_Nigra: i32 = 27;
pub const Left_VentralDC: i32 = 28;
pub const Left_undetermined: i32 = 29;
pub const Left_vessel: i32 = 30;
pub const Left_choroid_plexus: i32 = 31;
pub const Left_F3orb: i32 = 32;
pub const Left_lOg: i32 = 33;
pub const Left_aOg: i32 = 34;
pub const Left_mOg: i32 = 35;
pub const Left_pOg: i32 = 36;
pub const Left_Stellate: i32 = 37;
pub const Left_Porg: i32 = 38;
pub const Left_Aorg: i32 = 39;
pub const Right_Cerebral_Exterior: i32 = 40;
pub const Right_Cerebral_White_Matter: i32 = 41;
pub const Right_Cerebral_Cortex: i32 = 42;
pub const Right_Lateral_Ventricle: i32 = 43;
pub const Right_Inf_Lat_Vent: i32 = 44;
pub const Right_Cerebellum_Exterior: i32 = 45;
pub const Right_Cerebellum_White_Matter: i32 = 46;
pub const Right_Cerebellum_Cortex: i32 = 47;
pub const Right_Thalamus_Proper: i32 = 49;
pub const Right_Thalamus: i32 = 49;
pub const Right_Caudate: i32 = 50;
pub const Right_Putamen: i32 = 51;
pub const Right_Pallidum: i32 = 52;
pub const Right_Hippocampus: i32 = 53;
pub const Right_Amygdala: i32 = 54;
pub const Right_Insula: i32 = 55;
pub const Right_Operculum: i32 = 56;
pub const Right_Lesion: i32 = 57;
pub const Right_Accumbens_area: i32 = 58;
pub const Right_Substancia_Nigra: i32 = 59;
pub const Right_VentralDC: i32 = 60;
pub const Right_undetermined: i32 = 61;
pub const Right_vessel: i32 = 62;
pub const Right_choroid_plexus: i32 = 63;
pub const Right_F3orb: i32 = 64;
pub const Right_lOg: i32 = 65;
pub const Right_aOg: i32 = 66;
pub const Right_mOg: i32 = 67;
pub const Right_pOg: i32 = 68;
pub const Right_Stellate: i32 = 69;
pub const Right_Porg: i32 = 70;
pub const Right_Aorg: i32 = 71;
pub const Fifth_Ventricle: i32 = 72;
pub const Left_Interior: i32 = 73;
pub const Right_Interior: i32 = 74;
pub const WM_hypointensities: i32 = 77;
pub const Left_WM_hypointensities: i32 = 78;
pub const Right_WM_hypointensities: i32 = 79;
pub const non_WM_hypointensities: i32 = 80;
pub const Left_non_WM_hypointensities: i32 = 81;
pub const Right_non_WM_hypointensities: i32 = 82;
pub const Left_F1: i32 = 83;
pub const Right_F1: i32 = 84;
pub const Optic_Chiasm: i32 = 85;
pub const Left_future_WMSA: i32 = 87;
pub const Right_future_WMSA: i32 = 88;
pub const future_WMSA: i32 = 89;
pub const Left_Amygdala_Anterior: i32 = 96;
pub const Right_Amygdala_Anterior: i32 = 97;
pub const Left_Claustrum: i32 = 138;
pub const Right_Claustrum: i32 = 139;
pub const Dura: i32 = 98;
pub const Epidermis: i32 = 118;
pub const Conn_Tissue: i32 = 119;
pub const SC_FAT_MUSCLE: i32 = 120;
pub const Cranium: i32 = 121;
pub const CSF_SA: i32 = 122;
pub const Muscle: i32 = 123;
pub const Ear: i32 = 124;
pub const Fatty_Tissue: i32 = 125;
pub const Spinal_Cord: i32 = 126;
pub const Soft_Tissue: i32 = 127;
pub const Nerve: i32 = 128;
pub const Bone: i32 = 129;
pub const Air: i32 = 130;
pub const Orbit: i32 = 131;
pub const Tongue: i32 = 132;
pub const Nasal_Structures: i32 = 133;
pub const Globe: i32 = 134;
pub const Teeth: i32 = 135;
pub const Skull: i32 = 165;
pub const Right_Temporal_Cerebral_White_Matter: i32 = 186;
pub const Left_Temporal_Cerebral_White_Matter: i32 = 187;
pub const Fat: i32 = 189;
pub const Bright_Unknown: i32 = 190;
pub const Dark_Unknown: i32 = 191;
pub const Corpus_Callosum: i32 = 192;
pub const alveus: i32 = 201;
pub const perforant_pathway: i32 = 202;
pub const parasubiculum: i32 = 203;
pub const presubiculum: i32 = 204;
pub const subiculum: i32 = 205;
pub const CA1: i32 = 206;
pub const CA2: i32 = 207;
pub const CA3: i32 = 208;
pub const CA4: i32 = 209;
pub const GC_DG: i32 = 210;
pub const HATA: i32 = 211;
pub const fimbria: i32 = 212;
pub const lateral_ventricle: i32 = 213;
pub const molecular_layer_HP: i32 = 214;
pub const hippocampal_fissure: i32 = 215;
pub const entorhinal_cortex: i32 = 216;
pub const molecular_layer_subiculum: i32 = 217;
pub const Amygdala: i32 = 218;
pub const Cerebral_White_Matter: i32 = 219;
pub const Cerebral_Cortex: i32 = 220;
pub const Inf_Lat_Vent: i32 = 221;
pub const Left_hippocampal_fissure: i32 = 193;
pub const Left_CADG_head: i32 = 194;
pub const Left_subiculum: i32 = 195;
pub const Left_fimbria: i32 = 196;
pub const Right_hippocampal_fissure: i32 = 197;
pub const Right_CADG_head: i32 = 198;
pub const Right_subiculum: i32 = 199;
pub const Right_fimbria: i32 = 200;
pub const Fornix: i32 = 250;
pub const CC_Posterior: i32 = 251;
pub const CC_Mid_Posterior: i32 = 252;
pub const CC_Central: i32 = 253;
pub const CC_Mid_Anterior: i32 = 254;
pub const CC_Anterior: i32 = 255;
pub const ctx_lh_infragranular: i32 = 270;
pub const ctx_lh_layer1: i32 = 271;
pub const ctx_lh_layer2: i32 = 272;
pub const ctx_lh_layer3: i32 = 273;
pub const ctx_lh_layer4: i32 = 274;
pub const ctx_lh_layer5: i32 = 275;
pub const ctx_lh_layer6: i32 = 276;
pub const ctx_lh_supragranular: i32 = 277;
pub const VOXEL_UNCHANGED: i32 = 256;
pub const CSF_ExtraCerebral: i32 = 257;
pub const Head_ExtraCerebral: i32 = 258;
pub const Aorta: i32 = 331;
pub const Left_Common_IliacA: i32 = 332;
pub const Right_Common_IliacA: i32 = 333;
pub const Left_External_IliacA: i32 = 334;
pub const Right_External_IliacA: i32 = 335;
pub const Left_Internal_IliacA: i32 = 336;
pub const Right_Internal_IliacA: i32 = 337;
pub const Left_Lateral_SacralA: i32 = 338;
pub const Right_Lateral_SacralA: i32 = 339;
pub const Left_ObturatorA: i32 = 340;
pub const Right_ObturatorA: i32 = 341;
pub const Left_Internal_PudendalA: i32 = 342;
pub const Right_Internal_PudendalA: i32 = 343;
pub const Left_UmbilicalA: i32 = 344;
pub const Right_UmbilicalA: i32 = 345;
pub const Left_Inf_RectalA: i32 = 346;
pub const Right_Inf_RectalA: i32 = 347;
pub const Left_Common_IliacV: i32 = 348;
pub const Right_Common_IliacV: i32 = 349;
pub const Left_External_IliacV: i32 = 350;
pub const Right_External_IliacV: i32 = 351;
pub const Left_Internal_IliacV: i32 = 352;
pub const Right_Internal_IliacV: i32 = 353;
pub const Left_ObturatorV: i32 = 354;
pub const Right_ObturatorV: i32 = 355;
pub const Left_Internal_PudendalV: i32 = 356;
pub const Right_Internal_PudendalV: i32 = 357;
pub const Pos_Lymph: i32 = 358;
pub const Neg_Lymph: i32 = 359;
pub const ctx_rh_infragranular: i32 = 370;
pub const ctx_rh_layer1: i32 = 371;
pub const ctx_rh_layer2: i32 = 372;
pub const ctx_rh_layer3: i32 = 373;
pub const ctx_rh_layer4: i32 = 374;
pub const ctx_rh_layer5: i32 = 375;
pub const ctx_rh_layer6: i32 = 376;
pub const ctx_rh_supragranular: i32 = 377;
pub const BA17: i32 = 400;
pub const BA18: i32 = 401;
pub const BA44: i32 = 402;
pub const BA45: i32 = 403;
pub const BA4a: i32 = 404;
pub const BA4p: i32 = 405;
pub const BA6: i32 = 406;
pub const BA2: i32 = 407;
pub const BAun1: i32 = 408;
pub const BAun2: i32 = 409;
pub const right_CA2_3: i32 = 500;
pub const right_alveus: i32 = 501;
pub const right_CA1: i32 = 502;
pub const right_fimbria: i32 = 503;
pub const right_presubiculum: i32 = 504;
pub const right_hippocampal_fissure: i32 = 505;
pub const right_CA4_DG: i32 = 506;
pub const right_subiculum: i32 = 507;
pub const right_fornix: i32 = 508;
pub const left_CA2_3: i32 = 550;
pub const left_alveus: i32 = 551;
pub const left_CA1: i32 = 552;
pub const left_fimbria: i32 = 553;
pub const left_presubiculum: i32 = 554;
pub const left_hippocampal_fissure: i32 = 555;
pub const left_CA4_DG: i32 = 556;
pub const left_subiculum: i32 = 557;
pub const left_fornix: i32 = 558;
pub const Tumor: i32 = 600;
pub const Cbm_Left_I_IV: i32 = 601;
pub const Cbm_Right_I_IV: i32 = 602;
pub const Cbm_Left_V: i32 = 603;
pub const Cbm_Right_V: i32 = 604;
pub const Cbm_Left_VI: i32 = 605;
pub const Cbm_Vermis_VI: i32 = 606;
pub const Cbm_Right_VI: i32 = 607;
pub const Cbm_Left_CrusI: i32 = 608;
pub const Cbm_Vermis_CrusI: i32 = 609;
pub const Cbm_Right_CrusI: i32 = 610;
pub const Cbm_Left_CrusII: i32 = 611;
pub const Cbm_Vermis_CrusII: i32 = 612;
pub const Cbm_Right_CrusII: i32 = 613;
pub const Cbm_Left_VIIb: i32 = 614;
pub const Cbm_Vermis_VIIb: i32 = 615;
pub const Cbm_Right_VIIb: i32 = 616;
pub const Cbm_Left_VIIIa: i32 = 617;
pub const Cbm_Vermis_VIIIa: i32 = 618;
pub const Cbm_Right_VIIIa: i32 = 619;
pub const Cbm_Left_VIIIb: i32 = 620;
pub const Cbm_Vermis_VIIIb: i32 = 621;
pub const Cbm_Right_VIIIb: i32 = 622;
pub const Cbm_Left_IX: i32 = 623;
pub const Cbm_Vermis_IX: i32 = 624;
pub const Cbm_Right_IX: i32 = 625;
pub const Cbm_Left_X: i32 = 626;
pub const Cbm_Vermis_X: i32 = 627;
pub const Cbm_Right_X: i32 = 628;
pub const SUSPICIOUS: i32 = 999;
pub const fmajor: i32 = 5100;
pub const fminor: i32 = 5101;
pub const cc_body: i32 = 5102;
pub const cc_bodyc: i32 = 5103;
pub const cc_bodypf: i32 = 5104;
pub const cc_bodypm: i32 = 5105;
pub const cc_bodyp: i32 = 5106;
pub const cc_bodyt: i32 = 5107;
pub const cc_genu: i32 = 5108;
pub const cc_rostrum: i32 = 5109;
pub const cc_splenium: i32 = 5110;
pub const acomm: i32 = 5111;
pub const mcp: i32 = 5112;
pub const lh_atr: i32 = 5200;
pub const lh_cab: i32 = 5201;
pub const lh_ccg: i32 = 5202;
pub const lh_cst: i32 = 5203;
pub const lh_ilf: i32 = 5204;
pub const lh_slfp: i32 = 5205;
pub const lh_slft: i32 = 5206;
pub const lh_unc: i32 = 5207;
pub const lh_cb: i32 = 5208;
pub const lh_slf: i32 = 5209;
pub const lh_af: i32 = 5210;
pub const lh_ifof: i32 = 5211;
pub const lh_fx: i32 = 5212;
pub const lh_fat: i32 = 5213;
pub const lh_or: i32 = 5214;
pub const lh_mlf: i32 = 5215;
pub const lh_slf1: i32 = 5216;
pub const lh_slf2: i32 = 5217;
pub const lh_slf3: i32 = 5218;
pub const lh_afd: i32 = 5219;
pub const lh_afv: i32 = 5220;
pub const lh_ar: i32 = 5221;
pub const lh_cbd: i32 = 5222;
pub const lh_cbv: i32 = 5223;
pub const lh_emc: i32 = 5224;
pub const lh_uf: i32 = 5225;
pub const rh_atr: i32 = 5300;
pub const rh_cab: i32 = 5301;
pub const rh_ccg: i32 = 5302;
pub const rh_cst: i32 = 5303;
pub const rh_ilf: i32 = 5304;
pub const rh_slfp: i32 = 5305;
pub const rh_slft: i32 = 5306;
pub const rh_unc: i32 = 5307;
pub const rh_cb: i32 = 5308;
pub const rh_slf: i32 = 5309;
pub const rh_af: i32 = 5310;
pub const rh_ifof: i32 = 5311;
pub const rh_fx: i32 = 5312;
pub const rh_fat: i32 = 5313;
pub const rh_or: i32 = 5314;
pub const rh_mlf: i32 = 5315;
pub const rh_slf1: i32 = 5316;
pub const rh_slf2: i32 = 5317;
pub const rh_slf3: i32 = 5318;
pub const rh_afd: i32 = 5319;
pub const rh_afv: i32 = 5320;
pub const rh_ar: i32 = 5321;
pub const rh_cbd: i32 = 5322;
pub const rh_cbv: i32 = 5323;
pub const rh_emc: i32 = 5324;
pub const rh_uf: i32 = 5325;
pub const wm_lh_unknown: i32 = 3000;
pub const wm_rh_unknown: i32 = 4000;
pub const Left_Unsegmented_WM: i32 = 5001;
pub const Right_Unsegmented_WM: i32 = 5002;
pub const MIN_CORTICAL_PARCELLATION: i32 = 1000;
pub const ctx_lh_unknown: i32 = 1000;
pub const ctx_lh_bankssts: i32 = 1001;
pub const ctx_lh_caudalanteriorcingulate: i32 = 1002;
pub const ctx_lh_caudalmiddlefrontal: i32 = 1003;
pub const ctx_lh_corpuscallosum: i32 = 1004;
pub const ctx_lh_cuneus: i32 = 1005;
pub const ctx_lh_entorhinal: i32 = 1006;
pub const ctx_lh_fusiform: i32 = 1007;
pub const ctx_lh_inferiorparietal: i32 = 1008;
pub const ctx_lh_inferiortemporal: i32 = 1009;
pub const ctx_lh_isthmuscingulate: i32 = 1010;
pub const ctx_lh_lateraloccipital: i32 = 1011;
pub const ctx_lh_lateralorbitofrontal: i32 = 1012;
pub const ctx_lh_lingual: i32 = 1013;
pub const ctx_lh_medialorbitofrontal: i32 = 1014;
pub const ctx_lh_middletemporal: i32 = 1015;
pub const ctx_lh_parahippocampal: i32 = 1016;
pub const ctx_lh_paracentral: i32 = 1017;
pub const ctx_lh_parsopercularis: i32 = 1018;
pub const ctx_lh_parsorbitalis: i32 = 1019;
pub const ctx_lh_parstriangularis: i32 = 1020;
pub const ctx_lh_pericalcarine: i32 = 1021;
pub const ctx_lh_postcentral: i32 = 1022;
pub const ctx_lh_posteriorcingulate: i32 = 1023;
pub const ctx_lh_precentral: i32 = 1024;
pub const ctx_lh_precuneus: i32 = 1025;
pub const ctx_lh_rostralanteriorcingulate: i32 = 1026;
pub const ctx_lh_rostralmiddlefrontal: i32 = 1027;
pub const ctx_lh_superiorfrontal: i32 = 1028;
pub const ctx_lh_superiorparietal: i32 = 1029;
pub const ctx_lh_superiortemporal: i32 = 1030;
pub const ctx_lh_supramarginal: i32 = 1031;
pub const ctx_lh_frontalpole: i32 = 1032;
pub const ctx_lh_temporalpole: i32 = 1033;
pub const ctx_lh_transversetemporal: i32 = 1034;
pub const ctx_lh_insula: i32 = 1035;
pub const ctx_rh_unknown: i32 = 2000;
pub const ctx_rh_bankssts: i32 = 2001;
pub const ctx_rh_caudalanteriorcingulate: i32 = 2002;
pub const ctx_rh_caudalmiddlefrontal: i32 = 2003;
pub const ctx_rh_corpuscallosum: i32 = 2004;
pub const ctx_rh_cuneus: i32 = 2005;
pub const ctx_rh_entorhinal: i32 = 2006;
pub const ctx_rh_fusiform: i32 = 2007;
pub const ctx_rh_inferiorparietal: i32 = 2008;
pub const ctx_rh_inferiortemporal: i32 = 2009;
pub const ctx_rh_isthmuscingulate: i32 = 2010;
pub const ctx_rh_lateraloccipital: i32 = 2011;
pub const ctx_rh_lateralorbitofrontal: i32 = 2012;
pub const ctx_rh_lingual: i32 = 2013;
pub const ctx_rh_medialorbitofrontal: i32 = 2014;
pub const ctx_rh_middletemporal: i32 = 2015;
pub const ctx_rh_parahippocampal: i32 = 2016;
pub const ctx_rh_paracentral: i32 = 2017;
pub const ctx_rh_parsopercularis: i32 = 2018;
pub const ctx_rh_parsorbitalis: i32 = 2019;
pub const ctx_rh_parstriangularis: i32 = 2020;
pub const ctx_rh_pericalcarine: i32 = 2021;
pub const ctx_rh_postcentral: i32 = 2022;
pub const ctx_rh_posteriorcingulate: i32 = 2023;
pub const ctx_rh_precentral: i32 = 2024;
pub const ctx_rh_precuneus: i32 = 2025;
pub const ctx_rh_rostralanteriorcingulate: i32 = 2026;
pub const ctx_rh_rostralmiddlefrontal: i32 = 2027;
pub const ctx_rh_superiorfrontal: i32 = 2028;
pub const ctx_rh_superiorparietal: i32 = 2029;
pub const ctx_rh_superiortemporal: i32 = 2030;
pub const ctx_rh_supramarginal: i32 = 2031;
pub const ctx_rh_frontalpole: i32 = 2032;
pub const ctx_rh_temporalpole: i32 = 2033;
pub const ctx_rh_transversetemporal: i32 = 2034;
pub const ctx_rh_insula: i32 = 2035;
pub const MAX_OUTLINE_CLAIMS: i32 = 10;
pub const CMA_FILL_NONE: i32 = 0;
pub const CMA_FILL_OUTLINE: i32 = 1;
pub const CMA_FILL_INTERIOR: i32 = 2;
pub const CSF_CLASS: i32 = 0;
pub const GM_CLASS: i32 = 1;
pub const WM_CLASS: i32 = 2;
pub const NTISSUE_CLASSES: i32 = 3;
pub const LH_CLASS: i32 = 4;
pub const RH_CLASS: i32 = 5;
pub const FLUID_CLASS: i32 = 6;
pub const OTHER_CLASS: i32 = 7;
/// `MAX_LABEL` (`rh_unc`).
pub const MAX_LABEL: i32 = rh_unc;
pub const MAX_CMA_LABEL: i32 = MAX_LABEL;
pub const MAX_CMA_LABELS: i32 = MAX_CMA_LABEL + 1;

/// `IS_FUTURE_WMSA(l)`.
pub fn is_future_wmsa<T: Label>(l: T) -> bool {
    (l == k(Left_future_WMSA)) || (l == k(Right_future_WMSA)) || (l == k(future_WMSA))
}

/// `IS_LH_LAYER(l)`.
pub fn is_lh_layer<T: Label>(l: T) -> bool {
    (l >= k(ctx_lh_infragranular)) && (l <= k(ctx_lh_supragranular))
}

/// `IS_CC(l)`.
pub fn is_cc<T: Label>(l: T) -> bool {
    l >= k(CC_Posterior) && l <= k(CC_Anterior)
}

/// `IS_RH_LAYER(l)`.
pub fn is_rh_layer<T: Label>(l: T) -> bool {
    (l >= k(ctx_rh_infragranular)) && (l <= k(ctx_rh_supragranular))
}

/// `IS_LAYER(l)`.
pub fn is_layer<T: Label>(l: T) -> bool {
    is_lh_layer(l) || is_rh_layer(l)
}

/// `IS_UNKNOWN(l)`.
pub fn is_unknown<T: Label>(l: T) -> bool {
    (l == k(Unknown)) || (l == k(255)) || (l == k(Bright_Unknown)) || (l == k(Dark_Unknown))
}

/// `IS_BRAIN(l)`.
pub fn is_brain<T: Label>(l: T) -> bool {
    (!is_unknown(l) && l < k(Dura)) || is_cc(l)
}

/// `IS_WM(l)`.
pub fn is_wm<T: Label>(l: T) -> bool {
    (l == k(Left_Cerebral_White_Matter))
        || (l == k(Right_Cerebral_White_Matter))
        || (l == k(Left_Temporal_Cerebral_White_Matter))
        || (l == k(Right_Temporal_Cerebral_White_Matter))
        || is_cc(l)
        || (l == k(Left_VentralDC))
        || (l == k(Right_VentralDC))
        || is_cerebellar_wm(l)
}

/// `IS_CEREBRAL_WM(l)`.
pub fn is_cerebral_wm<T: Label>(l: T) -> bool {
    (l == k(Left_Cerebral_White_Matter)) || (l == k(Right_Cerebral_White_Matter))
}

/// `IS_HYPO(l)`.
pub fn is_hypo<T: Label>(l: T) -> bool {
    (l == k(WM_hypointensities))
        || (l == k(Left_WM_hypointensities))
        || (l == k(Right_WM_hypointensities))
        || is_future_wmsa(l)
}

/// `IS_WMSA(l)`.
pub fn is_wmsa<T: Label>(l: T) -> bool {
    is_hypo(l)
}

/// `IS_WMH(l)`.
pub fn is_wmh<T: Label>(l: T) -> bool {
    is_wm(l) || is_hypo(l)
}

/// `IS_THALAMUS(l)`.
pub fn is_thalamus<T: Label>(l: T) -> bool {
    (l == k(Left_Thalamus))
        || (l == k(Left_Thalamus))
        || (l == k(Right_Thalamus))
        || (l == k(Right_Thalamus))
}

/// `IS_GM(l)`.
pub fn is_gm<T: Label>(l: T) -> bool {
    (l == k(Left_Cerebral_Cortex)) || (l == k(Right_Cerebral_Cortex))
}

/// `IS_VENTRAL_DC(l)`.
pub fn is_ventral_dc<T: Label>(l: T) -> bool {
    (l == k(Left_VentralDC)) || (l == k(Right_VentralDC))
}

/// `IS_CEREBELLAR_WM(l)`.
pub fn is_cerebellar_wm<T: Label>(l: T) -> bool {
    (l == k(Left_Cerebellum_White_Matter)) || (l == k(Right_Cerebellum_White_Matter))
}

/// `IS_CEREBELLAR_GM(l)`.
pub fn is_cerebellar_gm<T: Label>(l: T) -> bool {
    (l == k(Left_Cerebellum_Cortex)) || (l == k(Right_Cerebellum_Cortex))
}

/// `IS_CEREBELLUM(l)`.
pub fn is_cerebellum<T: Label>(l: T) -> bool {
    is_cerebellar_wm(l) || is_cerebellar_gm(l)
}

/// `IS_CHOROID(l)`.
pub fn is_choroid<T: Label>(l: T) -> bool {
    (l == k(Left_choroid_plexus)) || (l == k(Right_choroid_plexus))
}

/// `IS_HIPPO(l)`.
pub fn is_hippo<T: Label>(l: T) -> bool {
    (l == k(Left_Hippocampus)) || (l == k(Right_Hippocampus))
}

/// `IS_AMYGDALA(l)`.
pub fn is_amygdala<T: Label>(l: T) -> bool {
    (l == k(Left_Amygdala)) || (l == k(Right_Amygdala))
}

/// `IS_MTL(l)`.
pub fn is_mtl<T: Label>(l: T) -> bool {
    is_hippo(l) || is_amygdala(l)
}

/// `IS_CORTEX(l)`.
pub fn is_cortex<T: Label>(l: T) -> bool {
    (l == k(Left_Cerebral_Cortex)) || (l == k(Right_Cerebral_Cortex))
}

/// `IS_LAT_VENT(l)`.
pub fn is_lat_vent<T: Label>(l: T) -> bool {
    (l == k(Left_Lateral_Ventricle))
        || (l == k(Right_Lateral_Ventricle))
        || (l == k(Right_Inf_Lat_Vent))
        || (l == k(Left_Inf_Lat_Vent))
}

/// `IS_CSF(l)`.
pub fn is_csf<T: Label>(l: T) -> bool {
    is_lat_vent(l)
        || (l == k(CSF))
        || (l == k(CSF_SA))
        || (l == k(Third_Ventricle))
        || (l == k(Fourth_Ventricle))
}

/// `IS_INF_LAT_VENT(l)`.
pub fn is_inf_lat_vent<T: Label>(l: T) -> bool {
    (l == k(Left_Inf_Lat_Vent)) || (l == k(Right_Inf_Lat_Vent))
}

/// `IS_VENTRICLE(l)`.
pub fn is_ventricle<T: Label>(l: T) -> bool {
    is_lat_vent(l) || is_inf_lat_vent(l) || (l == k(Third_Ventricle)) || (l == k(Fourth_Ventricle))
}

/// `IS_CAUDATE(l)`.
pub fn is_caudate<T: Label>(l: T) -> bool {
    (l == k(Left_Caudate)) || (l == k(Right_Caudate))
}

/// `IS_PUTAMEN(l)`.
pub fn is_putamen<T: Label>(l: T) -> bool {
    (l == k(Left_Putamen)) || (l == k(Right_Putamen))
}

/// `IS_CLAUSTRUM(l)`.
pub fn is_claustrum<T: Label>(l: T) -> bool {
    (l == k(Left_Claustrum)) || (l == k(Right_Claustrum))
}

/// `IS_ACCUMBENS(l)`.
pub fn is_accumbens<T: Label>(l: T) -> bool {
    (l == k(Left_Accumbens_area)) || (l == k(Right_Accumbens_area))
}

/// `IS_PALLIDUM(l)`.
pub fn is_pallidum<T: Label>(l: T) -> bool {
    (l == k(Left_Pallidum)) || (l == k(Right_Pallidum))
}

/// `LABEL_WITH_NO_TOPOLOGY_CONSTRAINT(l)`.
pub fn label_with_no_topology_constraint<T: Label>(l: T) -> bool {
    (l == k(Right_non_WM_hypointensities))
        || (l == k(Left_non_WM_hypointensities))
        || (l == k(Left_WM_hypointensities))
        || (l == k(Right_WM_hypointensities))
        || (l == k(Left_choroid_plexus))
        || (l == k(Right_choroid_plexus))
        || (l == k(WM_hypointensities))
        || (l == k(Dura))
        || (l == k(Bone))
        || (l == k(CSF_SA))
        || (l == k(Epidermis))
        || (l == k(SC_FAT_MUSCLE))
        || (l == k(non_WM_hypointensities))
}

/// `IS_FIMBRIA(l)`.
pub fn is_fimbria<T: Label>(l: T) -> bool {
    l == k(left_fimbria) || l == k(right_fimbria) || l == k(fimbria)
}

/// `IS_GRAY_MATTER(l)`.
pub fn is_gray_matter<T: Label>(l: T) -> bool {
    (l == k(Left_Cerebral_Cortex))
        || (l == k(Left_Hippocampus))
        || (l == k(Left_Amygdala))
        || (l == k(Left_Putamen))
        || (l == k(Left_Caudate))
        || (l == k(Left_Cerebellum_Cortex))
        || (l == k(Left_Accumbens_area))
        || (l == k(Right_Cerebral_Cortex))
        || (l == k(Right_Hippocampus))
        || (l == k(Right_Amygdala))
        || (l == k(Right_Putamen))
        || (l == k(Right_Caudate))
        || (l == k(Right_Cerebellum_Cortex))
        || (l == k(Right_Accumbens_area))
}

/// `IS_WHITE_MATTER(l)`.
pub fn is_white_matter<T: Label>(l: T) -> bool {
    (l == k(Left_Cerebellum_White_Matter))
        || (l == k(Left_Cerebral_White_Matter))
        || (l == k(Right_Cerebellum_White_Matter))
        || (l == k(Right_Cerebral_White_Matter))
}

/// `IS_FLUID(l)`.
pub fn is_fluid<T: Label>(l: T) -> bool {
    (l == k(Left_Lateral_Ventricle))
        || (l == k(Left_Inf_Lat_Vent))
        || (l == k(Right_Lateral_Ventricle))
        || (l == k(Right_Inf_Lat_Vent))
        || (l == k(Unknown))
}

/// `IS_LH_CLASS(l)`.
pub fn is_lh_class<T: Label>(l: T) -> bool {
    (l == k(Left_Cerebral_Cortex))
        || (l == k(Left_Cerebral_White_Matter))
        || (l == k(Left_Hippocampus))
        || (l == k(Left_Amygdala))
        || (l == k(Left_Putamen))
        || (l == k(Left_Pallidum))
        || (l == k(Left_Caudate))
        || (l == k(Left_Lateral_Ventricle))
        || (l == k(Left_Inf_Lat_Vent))
        || (l == k(Left_Cerebellum_Cortex))
        || (l == k(Left_Cerebellum_White_Matter))
        || (l == k(Left_Thalamus))
        || (l == k(Left_vessel))
        || (l == k(Left_choroid_plexus))
        || (l == k(Left_VentralDC))
        || (l == k(Left_Accumbens_area))
}

/// `IS_RH_CLASS(l)`.
pub fn is_rh_class<T: Label>(l: T) -> bool {
    (l == k(Right_Cerebral_Cortex))
        || (l == k(Right_Cerebral_White_Matter))
        || (l == k(Right_Hippocampus))
        || (l == k(Right_Amygdala))
        || (l == k(Right_Putamen))
        || (l == k(Right_Pallidum))
        || (l == k(Right_Caudate))
        || (l == k(Right_Lateral_Ventricle))
        || (l == k(Right_Inf_Lat_Vent))
        || (l == k(Right_Cerebellum_Cortex))
        || (l == k(Right_Cerebellum_White_Matter))
        || (l == k(Right_Thalamus))
        || (l == k(Right_vessel))
        || (l == k(Right_choroid_plexus))
        || (l == k(Right_VentralDC))
        || (l == k(Right_Accumbens_area))
}

/// `IS_GRAY_CLASS(l)`.
pub fn is_gray_class<T: Label>(l: T) -> bool {
    is_gm(l) || (l == k(Left_Caudate)) || (l == k(Right_Caudate))
}

/// `IS_WHITE_CLASS(l)`.
pub fn is_white_class<T: Label>(l: T) -> bool {
    (l == k(Left_Cerebral_White_Matter)) || (l == k(Right_Cerebral_White_Matter))
}

/// `IS_CSF_CLASS(l)`.
pub fn is_csf_class<T: Label>(l: T) -> bool {
    (l == k(Left_Lateral_Ventricle))
        || (l == k(Right_Lateral_Ventricle))
        || (l == k(CSF))
        || (l == k(CSF_SA))
        || (l == k(Third_Ventricle))
        || (l == k(Fourth_Ventricle))
        || (l == k(Fifth_Ventricle))
        || (l == k(left_hippocampal_fissure))
        || (l == k(right_hippocampal_fissure))
        || (l == k(hippocampal_fissure))
}

/// `IS_CLASS(l, c)`: `IS_CSF_CLASS(l)` for `CSF_CLASS`, `IS_GRAY_CLASS(l)` for `GM_CLASS`, else
/// `IS_WHITE_CLASS(l)`.
pub fn is_class<T: Label>(l: T, c: i32) -> bool {
    if c == CSF_CLASS {
        is_csf_class(l)
    } else if c == GM_CLASS {
        is_gray_class(l)
    } else {
        is_white_class(l)
    }
}

/// C's `(int)` of a `float` voxel value or list entry (FS-311): truncated toward zero; NaN and
/// values outside the `int` range give -2147483648 under `reproduce` (x86-64), else NaN 0 and the
/// range's end, noted.
fn label_of(g: &mut Globals, v: f32, what: &str) -> i32 {
    if (-2147483648.0..2147483648.0).contains(&v) {
        return v as i32;
    }
    g.notices.add(
        &FS_311,
        format!("{what}: the value {v} converted to an int label"),
    );
    if g.notices.reproduce() {
        return i32::MIN;
    }
    if v.is_nan() {
        0
    } else if v > 0.0 {
        i32::MAX
    } else {
        i32::MIN
    }
}

/// `MRIgetVoxVal(mri, c, r, s, f)` as an `int` label (FS-311).
fn voxel_label(
    g: &mut Globals,
    err: &mut dyn Write,
    mri: &Mri,
    (c, r, s, f): (i32, i32, i32, i32),
    what: &str,
) -> Result<i32, Fatal> {
    let v = mri_get_vox_val(g, err, mri, c, r, s, f, 0)?;
    Ok(label_of(g, v, what))
}

/// `IsSubCorticalGray(SegId)`: 1 for the thalami, caudates, putamens, pallidums, hippocampi,
/// amygdalae, accumbens areas, ventral diencephalons and substantiae nigrae, else 0.
pub fn is_sub_cortical_gray(seg_id: i32) -> i32 {
    let list = [
        Left_Thalamus,
        Right_Thalamus,
        Left_Caudate,
        Right_Caudate,
        Left_Putamen,
        Right_Putamen,
        Left_Pallidum,
        Right_Pallidum,
        Left_Hippocampus,
        Right_Hippocampus,
        Left_Amygdala,
        Right_Amygdala,
        Left_Accumbens_area,
        Right_Accumbens_area,
        Left_VentralDC,
        Right_VentralDC,
        Left_Substancia_Nigra,
        Right_Substancia_Nigra,
    ];
    i32::from(list.contains(&seg_id))
}

/// The labels `SupraTentorialVolCorrection` counts outside the pial surface, as its tests list
/// them (`Left_Thalamus` and `Right_Thalamus` twice: FS-312).
const SUPRA_TENT: [i32; 32] = [
    Left_Lateral_Ventricle,
    Right_Lateral_Ventricle,
    Left_choroid_plexus,
    Right_choroid_plexus,
    Left_Inf_Lat_Vent,
    Right_Inf_Lat_Vent,
    WM_hypointensities,
    Left_WM_hypointensities,
    Right_WM_hypointensities,
    Left_Thalamus,
    Right_Thalamus,
    Left_Thalamus,
    Right_Thalamus,
    CC_Posterior,
    CC_Mid_Posterior,
    CC_Central,
    CC_Mid_Anterior,
    CC_Anterior,
    Left_VentralDC,
    Right_VentralDC,
    Left_Hippocampus,
    Right_Hippocampus,
    Left_Amygdala,
    Right_Amygdala,
    Left_Caudate,
    Right_Caudate,
    Left_Putamen,
    Right_Putamen,
    Left_Pallidum,
    Right_Pallidum,
    Left_Accumbens_area,
    Right_Accumbens_area,
];

/// The labels `CorticalGMVolCorrection`'s second method counts in the ribbon, as its tests list
/// them (the thalami twice: FS-312; `Left_vessel` without `Right_vessel`: FS-313).
const CTX_GM: [i32; 35] = [
    Left_Hippocampus,
    Right_Hippocampus,
    Left_Amygdala,
    Right_Amygdala,
    Left_Lateral_Ventricle,
    Right_Lateral_Ventricle,
    Left_choroid_plexus,
    Right_choroid_plexus,
    Left_Inf_Lat_Vent,
    Right_Inf_Lat_Vent,
    WM_hypointensities,
    Left_WM_hypointensities,
    Right_WM_hypointensities,
    Left_Thalamus,
    Right_Thalamus,
    Left_Thalamus,
    Right_Thalamus,
    CC_Posterior,
    CC_Mid_Posterior,
    CC_Central,
    CC_Mid_Anterior,
    CC_Anterior,
    Left_VentralDC,
    Right_VentralDC,
    Left_Caudate,
    Right_Caudate,
    Left_Putamen,
    Right_Putamen,
    Left_Pallidum,
    Right_Pallidum,
    Left_Accumbens_area,
    Right_Accumbens_area,
    Left_Cerebellum_Cortex,
    Right_Cerebellum_Cortex,
    Left_vessel,
];

/// How many times the tests of `list` add a voxel of label `id`: FreeSurfer's count under
/// `reproduce`; otherwise once at most (FS-312), `Right_vessel` added to `CorticalGMVolCorrection`'s
/// (`vessel_fix`, FS-313), each correction noted.
fn times_counted(g: &mut Globals, list: &[i32], id: i32, vessel_fix: bool, what: &str) -> usize {
    let n = list.iter().filter(|&&l| l == id).count();
    if n > 1 {
        g.notices.add(
            &FS_312,
            format!("{what}: label {id} tested twice, its voxels counted twice"),
        );
        if !g.notices.reproduce() {
            return 1;
        }
    }
    if vessel_fix && id == Right_vessel {
        g.notices.add(
            &FS_313,
            format!(
                "{what}: Right_vessel voxels in the ribbon are not counted (Left_vessel ones are)"
            ),
        );
        if !g.notices.reproduce() {
            return 1;
        }
    }
    n
}

/// The voxel volume as `aseg->xsize * aseg->ysize * aseg->zsize` makes it (`float` products).
fn vox_size(m: &Mri) -> f64 {
    f64::from(m.vg.xsize * m.vg.ysize * m.vg.zsize)
}

/// `SupraTentorialVolCorrection(aseg, ribbon)`: the volume (in mm^3, the voxel volume as `float`
/// products of the segmentation's sizes) of the voxels of `aseg`'s extent that are not inside the
/// pial surface (`ribbon` at the voxel, as an `int`, not above 0) and whose label is a ventricle,
/// choroid plexus, hypointensity, thalamus (twice: FS-312), corpus callosum part, ventral
/// diencephalon, hippocampus, amygdala, caudate, putamen, pallidum or accumbens area; the sum in
/// `double`, voxel by voxel in column, row, slice order.
pub fn supra_tentorial_vol_correction(
    g: &mut Globals,
    err: &mut dyn Write,
    aseg: &Mri,
    ribbon: &Mri,
) -> Result<f64, Fatal> {
    const WHAT: &str = "SupraTentorialVolCorrection";
    let size = vox_size(aseg);
    let mut vol = 0.0f64;
    for c in 0..aseg.vg.width {
        for r in 0..aseg.vg.height {
            for s in 0..aseg.vg.depth {
                let ribbon_val = voxel_label(g, err, ribbon, (c, r, s, 0), WHAT)?;
                if ribbon_val > 0 {
                    continue;
                }
                let id = voxel_label(g, err, aseg, (c, r, s, 0), WHAT)?;
                for _ in 0..times_counted(g, &SUPRA_TENT, id, false, WHAT) {
                    vol += size;
                }
            }
        }
    }
    Ok(vol)
}

/// `CorticalGMVolCorrection(aseg, ribbon, hemi)`: the volume of the voxels of the ribbon of
/// `hemi` (1: `ribbon` 3, 2: 42, any other: every voxel) whose label is not cortex: the labels its
/// second method lists (hippocampi, amygdalae, ventricles, choroid plexus, hypointensities,
/// thalami twice (FS-312), corpus callosum, ventral diencephalons, caudates, putamens, pallidums,
/// accumbens areas, cerebellar cortex, `Left_vessel` (FS-313)). Prints
/// `CorticalGMVolCorrection(): hemi=<hemi>, vol1=<vol>, vol2=<vol2>` (`%g`) on standard output, `vol2`
/// the first method's volume (every label but 0, 2, 3, 41, 42).
pub fn cortical_gm_vol_correction(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    aseg: &Mri,
    ribbon: &Mri,
    hemi: i32,
) -> Result<f64, Fatal> {
    const WHAT: &str = "CorticalGMVolCorrection";
    let size = vox_size(aseg);
    let (mut vol, mut vol2) = (0.0f64, 0.0f64);
    for c in 0..aseg.vg.width {
        for r in 0..aseg.vg.height {
            for s in 0..aseg.vg.depth {
                let ribbon_val = voxel_label(g, err, ribbon, (c, r, s, 0), WHAT)?;
                if hemi == 1 && ribbon_val != 3 || hemi == 2 && ribbon_val != 42 {
                    continue;
                }
                let id = voxel_label(g, err, aseg, (c, r, s, 0), WHAT)?;
                if id != 3 && id != 42 && id != 2 && id != 41 && id != 0 {
                    vol2 += size;
                }
                for _ in 0..times_counted(g, &CTX_GM, id, true, WHAT) {
                    vol += size;
                }
            }
        }
    }
    let _ = out.write_all(
        format!(
            "CorticalGMVolCorrection(): hemi={hemi}, vol1={}, vol2={}\n",
            g_fmt(vol),
            g_fmt(vol2)
        )
        .as_bytes(),
    );
    Ok(vol)
}

/// `printf("%g", x)`.
fn g_fmt(x: f64) -> String {
    larmorx_freesurfer_core::cstd::format_double("%g", x).expect("%g is a double format")
}

/// `MRIasegContraLatLabel(id)`: the label of the same structure in the other hemisphere. 0 for 0;
/// above 999 the cortical and white-matter parcellations (1000-1998 and 2000-2998, 3000-3998 and
/// 4000-4998, 11100-11998 and 12100-12998, 13100-13998 and 14100-14998: 1000 added or taken),
/// 5001 and 5002, else -1 (no message); below, `cma.h`'s lateralized structures, the labels of no
/// side themselves, else -1 after `MRIasegContraLatLabel() id=<id> unrecognized` on standard output.
pub fn mri_aseg_contra_lat_label(out: &mut dyn Write, id: i32) -> i32 {
    if id == 0 {
        return 0;
    }
    if id > 999 {
        let mut id2 = -1;
        if (1000..1999).contains(&id) {
            id2 = id + 1000;
        }
        if (2000..2999).contains(&id) {
            id2 = id - 1000;
        }
        if (3000..3999).contains(&id) {
            id2 = id + 1000;
        }
        if (4000..4999).contains(&id) {
            id2 = id - 1000;
        }
        if (11100..11999).contains(&id) {
            id2 = id + 1000;
        }
        if (12100..12999).contains(&id) {
            id2 = id - 1000;
        }
        if (13100..13999).contains(&id) {
            id2 = id + 1000;
        }
        if (14100..14999).contains(&id) {
            id2 = id - 1000;
        }
        if id == 5001 {
            id2 = 5002;
        }
        if id == 5002 {
            id2 = 5001;
        }
        return id2;
    }
    #[allow(unreachable_patterns)]
    match id {
        Left_Cerebral_White_Matter => Right_Cerebral_White_Matter,
        Left_Cerebral_Exterior => Right_Cerebral_Exterior,
        Left_Cerebral_Cortex => Right_Cerebral_Cortex,
        Left_Lateral_Ventricle => Right_Lateral_Ventricle,
        Left_Inf_Lat_Vent => Right_Inf_Lat_Vent,
        Left_Cerebellum_Exterior => Right_Cerebellum_Exterior,
        Left_Cerebellum_White_Matter => Right_Cerebellum_White_Matter,
        Left_Cerebellum_Cortex => Right_Cerebellum_Cortex,
        Left_Thalamus => Right_Thalamus,
        9 => 48,
        Left_Caudate => Right_Caudate,
        Left_Putamen => Right_Putamen,
        Left_Pallidum => Right_Pallidum,
        Left_Hippocampus => Right_Hippocampus,
        Left_Amygdala => Right_Amygdala,
        Left_Insula => Right_Insula,
        Left_Operculum => Right_Operculum,
        Left_Lesion => Right_Lesion,
        Left_Accumbens_area => Right_Accumbens_area,
        Left_Substancia_Nigra => Right_Substancia_Nigra,
        Left_VentralDC => Right_VentralDC,
        Left_undetermined => Right_undetermined,
        Left_vessel => Right_vessel,
        Left_choroid_plexus => Right_choroid_plexus,
        Left_F3orb => Right_F3orb,
        Left_lOg => Right_lOg,
        Left_aOg => Right_aOg,
        Left_mOg => Right_mOg,
        Left_pOg => Right_pOg,
        Left_Stellate => Right_Stellate,
        Left_Porg => Right_Porg,
        Left_Aorg => Right_Aorg,
        Left_Interior => Right_Interior,
        Left_WM_hypointensities => Right_WM_hypointensities,
        Left_non_WM_hypointensities => Right_non_WM_hypointensities,
        Left_F1 => Right_F1,
        Left_Amygdala_Anterior => Right_Amygdala_Anterior,
        Right_Cerebral_Exterior => Left_Cerebral_Exterior,
        Right_Cerebral_White_Matter => Left_Cerebral_White_Matter,
        Right_Cerebral_Cortex => Left_Cerebral_Cortex,
        Right_Inf_Lat_Vent => Left_Inf_Lat_Vent,
        Right_Cerebellum_Exterior => Left_Cerebellum_Exterior,
        Right_Cerebellum_White_Matter => Left_Cerebellum_White_Matter,
        Right_Cerebellum_Cortex => Left_Cerebellum_Cortex,
        Right_Thalamus => Left_Thalamus,
        48 => 9,
        Right_Caudate => Left_Caudate,
        Right_Putamen => Left_Putamen,
        Right_Pallidum => Left_Pallidum,
        Right_Hippocampus => Left_Hippocampus,
        Right_Amygdala => Left_Amygdala,
        Right_Insula => Left_Insula,
        Right_Operculum => Left_Operculum,
        Right_Lesion => Left_Lesion,
        Right_Accumbens_area => Left_Accumbens_area,
        Right_Substancia_Nigra => Left_Substancia_Nigra,
        Right_VentralDC => Left_VentralDC,
        Right_undetermined => Left_undetermined,
        Right_vessel => Left_vessel,
        Right_choroid_plexus => Left_choroid_plexus,
        Right_F3orb => Left_F3orb,
        Right_lOg => Left_lOg,
        Right_aOg => Left_aOg,
        Right_mOg => Left_mOg,
        Right_pOg => Left_pOg,
        Right_Stellate => Left_Stellate,
        Right_Porg => Left_Porg,
        Right_Aorg => Left_Aorg,
        Right_Interior => Left_Interior,
        Right_Lateral_Ventricle => Left_Lateral_Ventricle,
        Right_WM_hypointensities => Left_WM_hypointensities,
        Right_non_WM_hypointensities => Left_non_WM_hypointensities,
        Right_F1 => Left_F1,
        Right_Amygdala_Anterior => Left_Amygdala_Anterior,
        138 => 139,
        139 => 138,
        265 => 266,
        266 => 265,
        853 => 853,
        883 => 883,
        865 => 866,
        866 => 865,
        819 => 820,
        820 => 819,
        821 => 822,
        822 => 821,
        843 => 844,
        844 => 843,
        869 => 870,
        870 => 869,
        72
        | Optic_Chiasm
        | Third_Ventricle
        | Fourth_Ventricle
        | Brain_Stem
        | CC_Posterior
        | CC_Mid_Posterior
        | CC_Central
        | CC_Mid_Anterior
        | CC_Anterior
        | CSF_ExtraCerebral
        | Head_ExtraCerebral
        | 165
        | 259
        | 260
        | 262
        | 502
        | 902
        | 514
        | 914
        | WM_hypointensities
        | non_WM_hypointensities
        | CSF => id,
        _ => {
            let _ = writeln!(out, "MRIasegContraLatLabel() id={id} unrecognized");
            -1
        }
    }
}

/// `MRIlrswapAseg(aseg)`: a clone of `aseg` (`MRIclone`) whose first frame holds each voxel's
/// contralateral label ([`mri_aseg_contra_lat_label`], the label itself where it gives -1).
pub fn mri_lrswap_aseg(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    aseg: &Mri,
) -> Result<Option<Mri>, Fatal> {
    let Some(mut swap) = mri_clone(g, out, err, aseg, None)?.into_new() else {
        return Ok(None);
    };
    for c in 0..aseg.vg.width {
        for r in 0..aseg.vg.height {
            for s in 0..aseg.vg.depth {
                let id = voxel_label(g, err, aseg, (c, r, s, 0), "MRIlrswapAseg")?;
                let mut id2 = mri_aseg_contra_lat_label(out, id);
                if id2 == -1 {
                    id2 = id;
                }
                mri_set_vox_val(g, err, &mut swap, c, r, s, 0, id2 as f32, 0)?;
            }
        }
    }
    Ok(Some(swap))
}

/// `lh->rptr[1][n]` (FS-315: beyond the matrix, an error in both modes).
fn list_entry(g: &mut Globals, err: &mut dyn Write, m: &Matrix, n: i32) -> Result<f32, Fatal> {
    match m.relt_index(1, n) {
        Some(_) => Ok(m.relt(1, n)),
        None => {
            g.notices.add(
                &FS_315,
                format!(
                    "MRIlrswapSeg: label {n} of a list of {} numbers",
                    m.rows * m.cols
                ),
            );
            log::error_exit(
                g,
                err,
                log::ERROR_BADPARM,
                "MRIlrswapSeg(): the label lists hold fewer numbers than rows (FreeSurfer reads past them)",
            )?;
            Ok(0.0)
        }
    }
}

/// `MRIlrswapSeg(seg, lh, rh)`: a clone of `seg` with its pulse parameters and colour table whose
/// first frame holds each voxel's label swapped by the lists (the first row of `lh` and `rh`, as
/// far as their row count: a voxel equal, as a `float`, to `lh`'s n-th number takes `rh`'s n-th,
/// one equal to `rh`'s takes `lh`'s, `lh` tested first at each n; others keep theirs);
/// `MRIlrswapSeg(): nswaps <count>` on standard output. Lists of different row counts: `None` after
/// `ERROR: MRIlrswapSeg(): lh=<rows> != rh=<rows>`. FreeSurfer's loop runs in threads; the voxels
/// are independent and the count a sum of integers, so the result does not depend on them.
pub fn mri_lrswap_seg(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    seg: &Mri,
    lh: &Matrix,
    rh: &Matrix,
) -> Result<Option<Mri>, Fatal> {
    if lh.rows != rh.rows {
        let _ = writeln!(
            out,
            "ERROR: MRIlrswapSeg(): lh={} != rh={}",
            lh.rows, rh.rows
        );
        return Ok(None);
    }
    let Some(mut swap) = mri_clone(g, out, err, seg, None)?.into_new() else {
        return Ok(None);
    };
    mri_copy_pulse_parameters(seg, &mut swap);
    if seg.ct.is_some() {
        swap.ct = ctab_deep_copy(g, err, seg.ct.as_ref());
    }
    let mut nswaps = 0i32;
    for c in 0..seg.vg.width {
        for r in 0..seg.vg.height {
            for s in 0..seg.vg.depth {
                let id = voxel_label(g, err, seg, (c, r, s, 0), "MRIlrswapSeg")?;
                let mut id2 = -1;
                for n in 1..=lh.rows {
                    let (l, rv) = (list_entry(g, err, lh, n)?, list_entry(g, err, rh, n)?);
                    if id as f32 == l {
                        id2 = label_of(g, rv, "MRIlrswapSeg");
                        nswaps = nswaps.wrapping_add(1);
                        break;
                    } else if id as f32 == rv {
                        id2 = label_of(g, l, "MRIlrswapSeg");
                        nswaps = nswaps.wrapping_add(1);
                        break;
                    }
                }
                if id2 == -1 {
                    id2 = id;
                }
                mri_set_vox_val(g, err, &mut swap, c, r, s, 0, id2 as f32, 0)?;
            }
        }
    }
    let _ = writeln!(out, "MRIlrswapSeg(): nswaps {nswaps}");
    Ok(Some(swap))
}

/// `MRIfixAsegWithRibbon(aseg, ribbon, asegfixed)`: `aseg` copied (`MRIcopy`, into `asegfixed`
/// when given), then each voxel labelled 0, 2, 3, 41 or 42 takes the ribbon's value (as an
/// `int`). `asegfixed` cannot be `aseg` itself here (FreeSurfer's copy onto itself is a no-op).
pub fn mri_fix_aseg_with_ribbon(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    aseg: &Mri,
    ribbon: &Mri,
    asegfixed: Option<&mut Mri>,
) -> Result<Ret<Mri>, Fatal> {
    const WHAT: &str = "MRIfixAsegWithRibbon";
    let mut fresh = None;
    let given = asegfixed.is_some();
    let d: &mut Mri = match asegfixed {
        Some(d) => {
            if mri_copy(g, out, err, aseg, Some(&mut *d))?.is_null() {
                return Ok(Ret::Null);
            }
            d
        }
        None => match mri_copy(g, out, err, aseg, None)?.into_new() {
            Some(m) => fresh.insert(m),
            None => return Ok(Ret::Null),
        },
    };
    for c in 0..aseg.vg.width {
        for r in 0..aseg.vg.height {
            for s in 0..aseg.vg.depth {
                let id = voxel_label(g, err, aseg, (c, r, s, 0), WHAT)?;
                if id == 2 || id == 41 || id == 3 || id == 42 || id == 0 {
                    let rid = voxel_label(g, err, ribbon, (c, r, s, 0), WHAT)?;
                    mri_set_vox_val(g, err, d, c, r, s, 0, rid as f32, 0)?;
                }
            }
        }
    }
    Ok(if given {
        Ret::Given
    } else {
        Ret::New(fresh.expect("the copy made above"))
    })
}

/// `MRIoneHotEncode(seg, segidlist, Force1)`: a `uchar` volume of `seg`'s extent with one frame
/// per listed label (`seg`'s header and pulse parameters), 1 in the frame of the first list entry
/// equal to the voxel's label (as an `int`), else, when `force1` is not 0 and the list holds 0, 1
/// in the frame of the last 0. `MRIoneHotEncode(): n=<n> Force1=<force1>: <labels, each followed
/// by a blank>` and a newline on standard output. `None` when the volume cannot be made.
pub fn mri_one_hot_encode(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    seg: &Mri,
    segidlist: &[i32],
    force1: i32,
) -> Result<Option<Mri>, Fatal> {
    let nsegs = segidlist.len() as i32;
    let (w, h, d) = (seg.vg.width, seg.vg.height, seg.vg.depth);
    let mut ohe = mri_alloc_sequence(g, out, err, w, h, d, MRI_UCHAR, nsegs)?;
    mri_copy_header(g, out, err, seg, Some(&mut ohe))?;
    mri_copy_pulse_parameters(seg, &mut ohe);
    let mut line = format!("MRIoneHotEncode(): n={nsegs} Force1={force1}: ");
    let mut segno0 = -1i32;
    for (n, &id) in segidlist.iter().enumerate() {
        line += &format!("{id} ");
        if id == 0 {
            segno0 = n as i32;
        }
    }
    line.push('\n');
    let _ = out.write_all(line.as_bytes());
    for c in 0..w {
        for r in 0..h {
            for s in 0..d {
                let id = voxel_label(g, err, seg, (c, r, s, 0), "MRIoneHotEncode")?;
                let hit = segidlist.iter().position(|&l| l == id);
                if let Some(n) = hit {
                    mri_set_vox_val(g, err, &mut ohe, c, r, s, n as i32, 1.0, 0)?;
                } else if segno0 != -1 && force1 != 0 {
                    mri_set_vox_val(g, err, &mut ohe, c, r, s, segno0, 1.0, 0)?;
                }
            }
        }
    }
    Ok(Some(ohe))
}

/// `brainVolumeStatsFilename`: the cache's name in a subject's folder.
pub const BRAIN_VOLUME_STATS_FILENAME: &str = "stats/brainvol.stats";

/// The measures of the cache's lines: the text a line must hold, the rest of its line.
const VOLUME_NAMES: [(&str, &str); 16] = [
    ("BrainSeg, BrainSegVol", "Brain Segmentation Volume"),
    (
        "BrainSegNotVent, BrainSegVolNotVent",
        "Brain Segmentation Volume Without Ventricles",
    ),
    ("SupraTentorial, SupraTentorialVol", "Supratentorial volume"),
    (
        "SupraTentorialNotVent, SupraTentorialVolNotVent",
        "Supratentorial volume",
    ),
    (
        "SubCortGray, SubCortGrayVol",
        "Subcortical gray matter volume",
    ),
    (
        "lhCortex, lhCortexVol",
        "Left hemisphere cortical gray matter volume",
    ),
    (
        "rhCortex, rhCortexVol",
        "Right hemisphere cortical gray matter volume",
    ),
    ("Cortex, CortexVol", "Total cortical gray matter volume"),
    ("TotalGray, TotalGrayVol", "Total gray matter volume"),
    (
        "lhCerebralWhiteMatter, lhCerebralWhiteMatterVol",
        "Left hemisphere cerebral white matter volume",
    ),
    (
        "rhCerebralWhiteMatter, rhCerebralWhiteMatterVol",
        "Right hemisphere cerebral white matter volume",
    ),
    (
        "CerebralWhiteMatter, CerebralWhiteMatterVol",
        "Total cerebral white matter volume",
    ),
    ("Mask, MaskVol", "Mask Volume"),
    (
        "SupraTentorialNotVentVox, SupraTentorialVolNotVentVox",
        "Supratentorial volume voxel count",
    ),
    (
        "BrainSegNotVentSurf, BrainSegVolNotVentSurf",
        "Brain Segmentation Volume Without Ventricles from Surf",
    ),
    (
        "VentricleChoroidVol, VentricleChoroidVol",
        "Volume of ventricles and choroid plexus",
    ),
];

/// `<subjdir>/<subject>/stats/brainvol.stats`, and where it is for the program (`dir`, its
/// working directory, for a relative name).
fn cache_name(dir: Option<&Path>, subject: &str, subjdir: &str) -> (String, std::path::PathBuf) {
    let name = format!("{subjdir}/{subject}/{BRAIN_VOLUME_STATS_FILENAME}");
    let path = match dir {
        Some(d) if !Path::new(&name).is_absolute() => d.join(&name),
        _ => std::path::PathBuf::from(&name),
    };
    (name, path)
}

/// `CacheBrainVolumeStats(stats, subject, subjdir)`: writes `<subjdir>/<subject>/stats/brainvol.stats`
/// (relative names from `dir`, the program's working directory): 16 lines `# Measure <names>,
/// <description>, <value>, mm^3`, the value as `std::fixed` with 12 decimals writes it (`%.12f`).
/// A file that cannot be made ends the program: `could not open file <name> for writing`.
pub fn cache_brain_volume_stats(
    g: &mut Globals,
    err: &mut dyn Write,
    dir: Option<&Path>,
    stats: &[f64; 16],
    subject: &str,
    subjdir: &str,
) -> Result<(), Fatal> {
    let (name, path) = cache_name(dir, subject, subjdir);
    let mut text = String::new();
    for (i, (names, what)) in VOLUME_NAMES.iter().enumerate() {
        text += &format!(
            "# Measure {names}, {what}, {}, mm^3\n",
            fmt_f(stats[i], 0, 12)
        );
    }
    let Ok(mut f) = std::fs::File::create(&path) else {
        log::fatal(
            g,
            err,
            1,
            &format!("could not open file {name} for writing"),
        )?;
        return Ok(());
    };
    // the C++ stream's write errors are not checked
    let _ = f.write_all(text.as_bytes());
    Ok(())
}

/// C's `isspace` in the "C" locale (the stream's white space).
fn is_c_space(b: u8) -> bool {
    matches!(b, b' ' | b'\t' | b'\n' | 0x0b | 0x0c | b'\r')
}

/// `std::stringstream(word) >> volume` (libstdc++'s `num_get` for `double` in the "C" locale):
/// after white space, an optional sign, digits with one `.`, and an exponent (`e` or `E` after a
/// digit, an optional sign, digits), as far as they go; then `strtod` of that text, which must take
/// all of it and must not overflow (an infinity fails). `None` when it fails.
fn stream_double(word: &[u8]) -> Option<f64> {
    let mut i = 0;
    while i < word.len() && is_c_space(word[i]) {
        i += 1;
    }
    let mut x = String::new();
    if let Some(&c) = word.get(i)
        && (c == b'+' || c == b'-')
    {
        x.push(c as char);
        i += 1;
    }
    let (mut found_mantissa, mut found_dec, mut found_sci) = (false, false, false);
    while let Some(&c) = word.get(i) {
        if c == b'0' {
            if !found_mantissa {
                x.push('0');
                found_mantissa = true;
            }
            i += 1;
        } else {
            break;
        }
    }
    while let Some(&c) = word.get(i) {
        if c == b'.' && !found_dec && !found_sci {
            x.push('.');
            found_dec = true;
        } else if c.is_ascii_digit() {
            x.push(c as char);
            found_mantissa = true;
        } else if (c == b'e' || c == b'E') && !found_sci && found_mantissa {
            x.push('e');
            found_sci = true;
            i += 1;
            match word.get(i) {
                Some(&s) if s == b'+' || s == b'-' => x.push(s as char),
                Some(_) => continue,
                None => break,
            }
        } else {
            break;
        }
        i += 1;
    }
    // what strtod takes whole: the text above is a number when it has a digit before any
    // exponent and, after an `e`, at least one digit
    let v: f64 = x.parse().ok()?;
    v.is_finite().then_some(v)
}

/// `ComputeBrainVolumeStats(subject, subjdir)`, the caller's (it reads the subject's volumes and
/// surfaces): the 16 statistics.
pub type ComputeStats<'a> =
    dyn FnMut(&mut Globals, &mut dyn Write, &mut dyn Write) -> Result<[f64; 16], Fatal> + 'a;

/// `ReadCachedBrainVolumeStats(subject, subjdir)`: the 16 statistics of
/// `<subjdir>/<subject>/stats/brainvol.stats` (relative names from `dir`). Each line that holds a
/// measure's names gives it the first of its words that, without its last character, reads as a
/// number ([`stream_double`]). A missing file: `computing (and caching) brain volume statistics` on
/// standard output, then `compute` (`ComputeBrainVolumeStats`) and [`cache_brain_volume_stats`]. A
/// statistic not found (or negative) ends the program: `could not find stat for <names> in
/// <name>`. FS-314: a measure's line with no number and white space at its end.
pub fn read_cached_brain_volume_stats(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    dir: Option<&Path>,
    subject: &str,
    subjdir: &str,
    compute: &mut ComputeStats<'_>,
) -> Result<[f64; 16], Fatal> {
    let (name, path) = cache_name(dir, subject, subjdir);
    let Ok(f) = std::fs::File::open(&path) else {
        let _ = out.write_all(b"computing (and caching) brain volume statistics\n");
        let stats = compute(g, out, err)?;
        cache_brain_volume_stats(g, err, dir, &stats, subject, subjdir)?;
        return Ok(stats);
    };
    let mut bytes = Vec::new();
    // a folder opens and reads nothing
    let _ = std::io::Read::read_to_end(&mut std::io::BufReader::new(f), &mut bytes);
    let mut stats = [-1.0f64; 16];
    let mut lines: Vec<&[u8]> = bytes.split(|&b| b == b'\n').collect();
    if bytes.last() == Some(&b'\n') || bytes.is_empty() {
        lines.pop();
    }
    for line in lines {
        for (i, (names, _)) in VOLUME_NAMES.iter().enumerate() {
            let names = names.as_bytes();
            if !line.windows(names.len()).any(|w| w == names) {
                continue;
            }
            let mut at = 0;
            loop {
                // `while (!ss.eof())`: the line's end was reached by the last word
                if at >= line.len() && at > 0 {
                    break;
                }
                while at < line.len() && is_c_space(line[at]) {
                    at += 1;
                }
                if at >= line.len() {
                    // `ss >> word` reads nothing: `word.pop_back()` of the empty word
                    g.notices.add(
                        &FS_314,
                        "ReadCachedBrainVolumeStats: a measure's line without a number ends in white space",
                    );
                    if g.notices.reproduce() {
                        log::fatal(
                            g,
                            err,
                            1,
                            "ReadCachedBrainVolumeStats: a line without a number ends in white space (FreeSurfer aborts here)",
                        )?;
                    }
                    break;
                }
                let start = at;
                while at < line.len() && !is_c_space(line[at]) {
                    at += 1;
                }
                let word = &line[start..at - 1];
                if let Some(v) = stream_double(word) {
                    stats[i] = v;
                    break;
                }
            }
        }
    }
    for (n, (names, _)) in VOLUME_NAMES.iter().enumerate() {
        if stats[n] < 0.0 {
            log::fatal(
                g,
                err,
                1,
                &format!("could not find stat for {names} in {name}"),
            )?;
        }
    }
    Ok(stats)
}

#[cfg(test)]
mod tests {
    use super::*;
    use larmorx_core::defects::UpstreamDefects;

    /// libstdc++'s `>> double` on words (the probe's `bvr` compares the reader as a whole).
    #[test]
    fn stream_double_takes_libstdcxx_numbers() {
        for (w, v) in [
            ("5", Some(5.0)),
            ("+7", Some(7.0)),
            ("-3.5", Some(-3.5)),
            (".5", Some(0.5)),
            ("5.", Some(5.0)),
            ("007.25", Some(7.25)),
            ("1.5.3", Some(1.5)),
            ("0x10", Some(0.0)),
            ("4e2x", Some(400.0)),
            ("1e-400", Some(0.0)),
            ("1e", None),
            ("1e+", None),
            ("1e400", None),
            ("-", None),
            ("", None),
            ("abc", None),
            ("inf", None),
            ("nan", None),
            ("e5", None),
        ] {
            assert_eq!(stream_double(w.as_bytes()), v, "{w:?}");
        }
    }

    /// A missing cache: the message, `compute`'s statistics, and the cache written (read back).
    #[test]
    fn a_missing_cache_is_computed_and_written() {
        let dir = std::env::temp_dir().join(format!("larmorx-cma-cache-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("s/stats")).unwrap();
        let mut g = Globals::new(UpstreamDefects::Reproduce);
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let want: [f64; 16] = std::array::from_fn(|i| i as f64 * 1.5 + 0.25);
        let mut calls = 0;
        let mut compute = |_: &mut Globals, _: &mut dyn Write, _: &mut dyn Write| {
            calls += 1;
            Ok(want)
        };
        let got = read_cached_brain_volume_stats(
            &mut g,
            &mut out,
            &mut err,
            Some(&dir),
            "s",
            ".",
            &mut compute,
        )
        .unwrap();
        assert_eq!((got, calls), (want, 1));
        assert_eq!(out, b"computing (and caching) brain volume statistics\n");
        let mut never =
            |_: &mut Globals, _: &mut dyn Write, _: &mut dyn Write| panic!("the cache exists now");
        let again = read_cached_brain_volume_stats(
            &mut g,
            &mut out,
            &mut err,
            Some(&dir),
            "s",
            ".",
            &mut never,
        )
        .unwrap();
        assert_eq!(again, want);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// FS-312 and FS-313: a thalamus voxel outside the ribbon counts twice under `reproduce`, once
    /// by default; a right vessel voxel in the right ribbon counts only by default.
    #[test]
    fn the_corrections_count_each_label_once_by_default() {
        for (mode, supra, ctx) in [
            (UpstreamDefects::Reproduce, 2.0, 0.0),
            (UpstreamDefects::Fix, 1.0, 1.0),
        ] {
            let mut g = Globals::new(mode);
            let (mut out, mut err) = (Vec::new(), Vec::new());
            let mut aseg =
                Mri::new(&mut g, &mut out, &mut err, &[2, 1, 1], MRI_UCHAR, true).unwrap();
            let mut ribbon =
                Mri::new(&mut g, &mut out, &mut err, &[2, 1, 1], MRI_UCHAR, true).unwrap();
            for m in [&mut aseg, &mut ribbon] {
                m.vg.xsize = 1.0;
                m.vg.ysize = 1.0;
                m.vg.zsize = 1.0;
            }
            mri_set_vox_val(
                &mut g,
                &mut err,
                &mut aseg,
                0,
                0,
                0,
                0,
                Left_Thalamus as f32,
                0,
            )
            .unwrap();
            mri_set_vox_val(
                &mut g,
                &mut err,
                &mut aseg,
                1,
                0,
                0,
                0,
                Right_vessel as f32,
                0,
            )
            .unwrap();
            mri_set_vox_val(&mut g, &mut err, &mut ribbon, 1, 0, 0, 0, 42.0, 0).unwrap();
            let s = supra_tentorial_vol_correction(&mut g, &mut err, &aseg, &ribbon).unwrap();
            let c =
                cortical_gm_vol_correction(&mut g, &mut out, &mut err, &aseg, &ribbon, 2).unwrap();
            assert_eq!((s, c), (supra, ctx), "{mode:?}");
            assert!(g.notices.contains(&FS_312) && g.notices.contains(&FS_313));
        }
    }
}
