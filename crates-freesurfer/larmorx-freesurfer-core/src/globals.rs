// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! FreeSurfer's process-wide state, one value per program run (`fs.core`, queue item 3.1-22).
//!
//! FreeSurfer keeps it in C globals: `Progname` (`utils/Progname.cpp`), `Gdiag`, `Gdiag_no`,
//! `Gx` ... `Gvz`, `IMAGE_SIZE` and `Gprofile` (`utils/diag.cpp` 47-68), `Gerror` and the error
//! log's settings (`utils/log.cpp` 55-58), `hips_cplxtor` and `hips_rtocplx`
//! (`utils/hips.cpp` 4-5), the C library's `errno`, and the static variables of the functions
//! that remember earlier calls (`DiagHeartbeat`, `DiagShowPctDone`, the random numbers of
//! `utils.cpp` and `OpenRan1`, `fastApproxAtan2f`, `GetVmPeak`, the logic-problem reports, the
//! debugging stack of `xDebug.cpp`, `ROMP_main_started`). The replica keeps them in
//! [`Globals`], which every translated function that reads or writes them takes, so that
//! programs run one after the other in one process (`recon-all`'s replica) or side by side in
//! tests each start from FreeSurfer's initial values.

use std::path::PathBuf;

use larmorx_core::defects::{Notices, UpstreamDefects};
use larmorx_core::drand48::Drand48;

use crate::random::Random;
use crate::utils::LogicReports;
use crate::xdebug::XDebug;

/// FreeSurfer's process-wide state at a program's start, with D13's choice and its notices.
#[derive(Clone, Debug)]
pub struct Globals {
    /// `Progname`: the program's name as messages print it; `"no name"` until the program sets
    /// it (`utils/Progname.cpp`).
    pub progname: String,
    /// `Gdiag`: the diagnostic bits (`unsigned long`, 64 bits on x86-64 Linux), [`crate::diag`].
    pub gdiag: u64,
    /// `Gdiag_no`, `Gdiag_no2`: an index the diagnostics print (-1: none).
    pub gdiag_no: i32,
    pub gdiag_no2: i32,
    /// `Gx`, `Gy`, `Gz` (and `Gx2` ... `Gz2`): the voxel the diagnostics follow (-1: none).
    pub gx: i32,
    pub gy: i32,
    pub gz: i32,
    pub gx2: i32,
    pub gy2: i32,
    pub gz2: i32,
    /// `Gsx`, `Gsy`, `Gsz`: a second diagnostic point (-1: none).
    pub gsx: i32,
    pub gsy: i32,
    pub gsz: i32,
    /// `Gvx`, `Gvy`, `Gvz`: a diagnostic vertex or voxel (-1: none).
    pub gvx: i32,
    pub gvy: i32,
    pub gvz: i32,
    /// `IMAGE_SIZE` (512).
    pub image_size: i32,
    /// `Gprofile` (0).
    pub gprofile: i32,
    /// `Gerror`: the code of the last `ErrorPrintf` or `ErrorExit` ([`crate::log`]).
    pub gerror: i32,
    /// The C library's `errno` as FreeSurfer's code would find it: the translated I/O sets it
    /// where the C library would, and `ErrorPrintf` prints its message when it is not 0.
    pub errno: i32,
    /// `setErrorLog`: the file every `fs::fatal` and `fs::error` message is appended to.
    pub error_log: Option<PathBuf>,
    /// `throwExceptions`: `fs::fatal` and `ErrorExit` throw their code instead of exiting.
    pub exceptions: bool,
    /// `hips_cplxtor`, `hips_rtocplx` (0; the HIPS library is gone, `utils/hips.cpp`).
    pub hips_cplxtor: i32,
    pub hips_rtocplx: i32,
    /// `DiagHeartbeat`'s `static float old_pct` (-10).
    pub(crate) heartbeat_old_pct: f32,
    /// `DiagShowPctDone`'s `static float old_pct` (-10).
    pub(crate) pct_done_old_pct: f32,
    /// The name the kernel gives the process (`/proc/self/comm`: the program file's first 15
    /// bytes, [`crate::random::comm_of`]), which `randomNumber` prints; empty until the program
    /// sets it (as on a system without `/proc`).
    pub comm: String,
    /// The random numbers' state (`utils.cpp`'s `idum`, `nrgcalls`, ...; `OpenRan1`'s).
    pub random: Random,
    /// `fastApproxAtan2f`'s `static` choice of `atan2f`, read at its first call.
    pub(crate) atan2f_choice: Option<bool>,
    /// `GetVmPeak`'s `static int u[5]`.
    pub(crate) vm_peak_usage: [i32; 5],
    /// The logic-problem reports' table (`reportEntry`, `copeWithLogicProblem2`).
    pub(crate) logic_reports: LogicReports,
    /// The debugging stack (`xDebug.cpp`).
    pub xdebug: XDebug,
    /// `ROMP_main_started`'s file and line.
    pub romp_main: Option<(String, i32)>,
    /// `use_new_MatricAlloc()`'s answer inverted: the variable `FREESURFER_MatrixAlloc_old` is
    /// set ([`crate::matrix`]; FreeSurfer reads it at the first allocation, no program sets it,
    /// so the program's start sets this from its environment).
    pub matrix_alloc_old: bool,
    /// The C library's 48-bit random numbers' process-wide state (`srand48`, `drand48`:
    /// `larmorx_core::drand48`, POSIX's definition; `PDFgaussian` draws from it).
    pub drand48: Drand48,
    /// `vg_isEqual_Threshold` (`transform.h`): the tolerance of `VOL_GEOM::operator==` and
    /// `vg_isEqual`, `FLT_EPSILON` until a program sets it (several set 1e-3).
    pub vg_is_equal_threshold: f64,
    /// `MRIIO_Strip_Pound` (`mri.h`: 1): `MRIgetVolumeName` also removes what follows a name's
    /// last `#`.
    pub mriio_strip_pound: i32,
    /// The DICOM format probe's process-wide state: CTN's condition stack and `IsDICOM`'s
    /// `static` answer for the last name it was asked (fs.io's `identify`).
    pub dicom: DicomState,
    /// `MRIsampleVolumeDerivativeSign` (`mri.cpp`): the sign the volume derivatives' magnitudes
    /// are multiplied by, 1 until `MRIsetSampleVolumeDerivativeSign` changes it.
    pub sample_volume_derivative_sign: f64,
    /// `mincFindVolume`'s static `count` (`transform.cpp`): 1 after a `minctracc` line named one
    /// `.mnc` volume, until a later line names another.
    pub minc_find_volume_count: i32,
    /// D13's choice and the notices of the defects that changed this run's results.
    pub notices: Notices,
}

/// The DICOM format probe's process-wide state (`utils/DICOMRead.cpp`'s `IsDICOM` and the CTN
/// library it calls).
#[derive(Clone, Debug, Default)]
pub struct DicomState {
    /// CTN's condition stack (and the C library's state its messages use).
    pub stack: larmorx_io::dicom::CondStack,
    /// `IsDICOM`'s `static char file[1024]` (empty at the start) and `static int yes`.
    pub is_dicom_file: Vec<u8>,
    pub is_dicom_yes: i32,
}

impl Globals {
    /// FreeSurfer's initial values, with D13's `mode`.
    pub fn new(mode: UpstreamDefects) -> Self {
        Globals {
            progname: "no name".to_owned(),
            gdiag: 0,
            gdiag_no: -1,
            gdiag_no2: -1,
            gx: -1,
            gy: -1,
            gz: -1,
            gx2: -1,
            gy2: -1,
            gz2: -1,
            gsx: -1,
            gsy: -1,
            gsz: -1,
            gvx: -1,
            gvy: -1,
            gvz: -1,
            image_size: 512,
            gprofile: 0,
            gerror: 0,
            errno: 0,
            error_log: None,
            exceptions: false,
            hips_cplxtor: 0,
            hips_rtocplx: 0,
            heartbeat_old_pct: -10.0,
            pct_done_old_pct: -10.0,
            comm: String::new(),
            random: Random::new(),
            atan2f_choice: None,
            vm_peak_usage: [0; 5],
            logic_reports: LogicReports::default(),
            xdebug: XDebug::default(),
            romp_main: None,
            matrix_alloc_old: false,
            drand48: Drand48::default(),
            vg_is_equal_threshold: f64::from(f32::EPSILON),
            mriio_strip_pound: 1,
            dicom: DicomState::default(),
            sample_volume_derivative_sign: 1.0,
            minc_find_volume_count: 0,
            notices: Notices::new(mode),
        }
    }

    /// D13's choice.
    pub fn mode(&self) -> UpstreamDefects {
        self.notices.mode()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn freesurfer_initial_values() {
        let g = Globals::new(UpstreamDefects::Fix);
        assert_eq!(g.progname, "no name");
        assert_eq!((g.gdiag, g.gdiag_no, g.gx, g.gvz), (0, -1, -1, -1));
        assert_eq!(
            (g.image_size, g.gprofile, g.gerror, g.errno),
            (512, 0, 0, 0)
        );
        assert!(!g.exceptions && g.error_log.is_none());
        assert_eq!(g.mode(), UpstreamDefects::Fix);
    }
}
