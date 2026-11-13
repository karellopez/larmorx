// SPDX-License-Identifier: Apache-2.0
//! The state every FreeSurfer program keeps for one run (spec §1), its output streams, its
//! environment table, the defects' mode and the notices it collected.

use std::io::Write;

use larmorx_core::defects::{Defect, Notices, UpstreamDefects};

use crate::env::Environment;

/// How a call that ends the program in FreeSurfer ended it here: the caller ends the run.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stop {
    /// The program ends with this exit status (0 to 255, as a shell reports it; a signal's death
    /// in `reproduce` is given as 128 + the signal, as a shell reports it).
    Exit(i32),
    /// FS-263's correction: with exceptions on, a fatal error's code reaches the caller that
    /// switched them on, after its message.
    Thrown(i32),
}

impl Stop {
    /// The exit status a program reports for this stop (a thrown code that no handler took ends
    /// the program as an uncaught exception would be corrected to: its code's low 8 bits).
    pub fn status(self) -> i32 {
        match self {
            Stop::Exit(s) => s,
            Stop::Thrown(c) => c & 0xff,
        }
    }
}

/// The diagnostic bit that switches the debug messages and the verbose start-up line on (§2, §4).
pub const DIAG_VERBOSE: u64 = 0x1000_0000;
/// The diagnostic bit of the heartbeat lines (§5).
pub const DIAG_HEARTBEAT: u64 = 0x4000;

/// FreeSurfer's error codes (§3).
pub mod codes {
    pub const NONE: i32 = 0;
    pub const NO_FILE: i32 = -1;
    pub const NO_MEMORY: i32 = -2;
    pub const UNSUPPORTED: i32 = -3;
    pub const BAD_PARM: i32 = -4;
    pub const BAD_FILE: i32 = -5;
    pub const SIZE: i32 = -6;
    pub const BAD_LOOP: i32 = -7;
    pub const OUT_OF_BOUNDS: i32 = -8;
}

/// The official build's directory, which its messages name (§3, §9, §12, §17).
pub const BUILD_ROOT: &str = "/home/rd521/space_freesurfer/build/docker_ubuntu22/trunk/rd521-8.2.0";

/// A FreeSurfer program run: the process-wide state of §1, its streams, environment and mode.
pub struct Process {
    /// The program's name for messages (`no name` until it sets one).
    pub progname: Vec<u8>,
    /// The diagnostic bits (`Gdiag`).
    pub gdiag: u64,
    /// The diagnostic index (`Gdiag_no`).
    pub gdiag_no: i32,
    /// The diagnostic voxel (`Gx`, `Gy`, `Gz`).
    pub gvoxel: [i32; 3],
    /// The second diagnostic point.
    pub gpoint2: [i32; 3],
    /// The diagnostic vertex (`Gvx`, `Gvy`, `Gvz`).
    pub gvertex: [i32; 3],
    /// The image size (`IMAGE_SIZE`).
    pub image_size: i32,
    /// The profiling level (`Gprofile`).
    pub gprofile: i32,
    /// The code of the last error (`Gerror`).
    pub gerror: i32,
    /// The C library's `errno` as FreeSurfer's calls leave it.
    pub errno: i32,
    /// The error log's file, when one is set.
    pub error_log: Option<Vec<u8>>,
    /// Whether fatal errors are turned into exceptions.
    pub throw_exceptions: bool,
    /// The environment the program reads.
    pub env: Environment,
    pub(crate) mode: UpstreamDefects,
    pub(crate) notices: Notices,
    out: Box<dyn Write + Send>,
    err: Box<dyn Write + Send>,
    pub(crate) random: crate::random::RandomState,
    pub(crate) heartbeat: crate::diag::Heartbeat,
    pub(crate) logic: crate::logic::LogicState,
    pub(crate) xdbg: crate::xdbg::DebugStack,
    pub(crate) atan_slow: Option<bool>,
    /// A fixed "now" (seconds since 1970) for callers and tests that need a reproducible
    /// clock; `None` reads the system clock.
    pub clock: Option<i64>,
    pub(crate) zone: Option<crate::timedate::ZoneCache>,
    pub(crate) vm_peak: [i32; 5],
}

impl Process {
    /// A new run with FreeSurfer's start values (§1), the given environment, defects' mode and
    /// output streams (standard output, standard error).
    pub fn new(
        env: Environment,
        mode: UpstreamDefects,
        out: Box<dyn Write + Send>,
        err: Box<dyn Write + Send>,
    ) -> Process {
        Process {
            progname: b"no name".to_vec(),
            gdiag: 0,
            gdiag_no: -1,
            gvoxel: [-1; 3],
            gpoint2: [-1; 3],
            gvertex: [-1; 3],
            image_size: 512,
            gprofile: 0,
            gerror: 0,
            errno: 0,
            error_log: None,
            throw_exceptions: false,
            env,
            mode,
            notices: Notices::new(mode),
            out,
            err,
            random: Default::default(),
            heartbeat: Default::default(),
            logic: Default::default(),
            xdbg: Default::default(),
            atan_slow: None,
            clock: None,
            zone: None,
            vm_peak: [0; 5],
        }
    }

    /// A run whose outputs are thrown away (for library callers that need no messages).
    pub fn silent(env: Environment, mode: UpstreamDefects) -> Process {
        Process::new(
            env,
            mode,
            Box::new(std::io::sink()),
            Box::new(std::io::sink()),
        )
    }

    /// The defects' mode of this run.
    pub fn mode(&self) -> UpstreamDefects {
        self.mode
    }

    /// The notices of the defects that changed (or, reproduced, would have changed) a result.
    pub fn notices(&self) -> &Notices {
        &self.notices
    }

    /// Takes the notices collected so far.
    pub fn take_notices(&mut self) -> Notices {
        std::mem::replace(&mut self.notices, Notices::new(self.mode))
    }

    /// Notes that `defect` changed this result (or would have).
    pub fn note(&mut self, defect: &'static Defect, detail: impl Into<String>) {
        self.notices.add(defect, detail);
    }

    /// Standard output.
    pub fn out(&mut self) -> &mut (dyn Write + Send) {
        &mut *self.out
    }

    /// Standard error (unbuffered in FreeSurfer: each message reaches it as it is written).
    pub fn err(&mut self) -> &mut (dyn Write + Send) {
        &mut *self.err
    }

    /// Writes `bytes` on standard output (errors of the stream are ignored, as C's are).
    pub fn put_out(&mut self, bytes: &[u8]) {
        let _ = self.out.write_all(bytes);
    }

    /// Writes `bytes` on standard error.
    pub fn put_err(&mut self, bytes: &[u8]) {
        let _ = self.err.write_all(bytes);
        let _ = self.err.flush();
    }

    /// Flushes both streams.
    pub fn flush(&mut self) {
        let _ = self.out.flush();
        let _ = self.err.flush();
    }
}
