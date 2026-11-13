// SPDX-License-Identifier: Apache-2.0
//! The environment record (spec §15): FreeSurfer's home, the subjects' folder, the user, the
//! working directory, the system, the colour table, the "now" stamp and the b-vector space,
//! with their messages on standard output; FS-08's unwritten values given defined ones.

use larmorx_core::defects::FS_08;

use crate::clib;
use crate::process::{Process, codes};
use crate::system;

/// The environment record (`FSENV`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FsEnv<C> {
    /// `FREESURFER_HOME`.
    pub freesurfer_home: Vec<u8>,
    /// `SUBJECTS_DIR`, when it is set.
    pub subjects_dir: Option<Vec<u8>>,
    /// The user's name, recorded only when `SUBJECTS_DIR` is set.
    pub user: Option<Vec<u8>>,
    /// The working directory (empty when it cannot be read: FS-08).
    pub cwd: Vec<u8>,
    /// The system's name, machine type and host name.
    pub sysname: Vec<u8>,
    pub machine: Vec<u8>,
    pub hostname: Vec<u8>,
    /// The "now" stamp of §13.
    pub date: Vec<u8>,
    /// 1 (scanner) or 2 (voxel).
    pub desired_bvec_space: i32,
    /// The colour table `$FREESURFER_HOME/FreeSurferColorLUT.txt`, as its reader gave it.
    pub ctab: C,
}

/// The reader of the colour table the record holds (`fs.labels`' reader; its own messages come
/// first, §2).
pub trait ColorTableReader {
    type Table;
    /// The table of `path`, `None` when it cannot be read.
    fn read(&mut self, p: &mut Process, path: &[u8]) -> Option<Self::Table>;
}

/// A reader that only opens the file and keeps its bytes (until `fs.labels`' reader exists): a
/// file that cannot be opened gives the reader's open error, `errno`'s text then
/// `CTABreadASCII(<file>): could not open file`, as C-style errors (§2).
#[derive(Clone, Copy, Debug, Default)]
pub struct OpenOnly;

impl ColorTableReader for OpenOnly {
    type Table = Vec<u8>;

    fn read(&mut self, p: &mut Process, path: &[u8]) -> Option<Vec<u8>> {
        match std::fs::read(crate::files::path_of(path)) {
            Ok(b) => Some(b),
            Err(e) => {
                p.errno = e.raw_os_error().unwrap_or(2);
                let mut m = b"CTABreadASCII(".to_vec();
                m.extend_from_slice(path);
                m.extend_from_slice(b"): could not open file");
                p.error_printf(codes::NO_FILE, &m);
                None
            }
        }
    }
}

impl Process {
    /// The environment record (`FSENVgetenv`), the colour table read by `reader`; `None` (after
    /// its message) when `FREESURFER_HOME` is unset, the colour table cannot be read, or the
    /// b-vector space is neither 1 nor 2.
    pub fn fsenv_getenv<R: ColorTableReader>(&mut self, reader: &mut R) -> Option<FsEnv<R::Table>> {
        let Some(home) = self.env.get("FREESURFER_HOME").map(<[u8]>::to_vec) else {
            self.put_out(b"FREESURFER_HOME not defined\n");
            return None;
        };
        let subjects_dir = self.env.get("SUBJECTS_DIR").map(<[u8]>::to_vec);
        let user = match subjects_dir {
            Some(_) => Some(self.user_name()),
            None => {
                self.put_out(b"SUBJECTS_DIR not defined\n");
                None
            }
        };
        let cwd = match system::working_directory() {
            Some(d) => d,
            None => {
                self.put_out(b"ERROR: getcwd returned no path");
                self.note(
                    &FS_08,
                    "the working directory could not be read: the record holds an empty one",
                );
                Vec::new()
            }
        };
        let sysname = system::system_name();
        let machine = system::machine_type();
        let hostname = system::host_name().unwrap_or_default();
        let mut lut = home.clone();
        lut.extend_from_slice(b"/FreeSurferColorLUT.txt");
        let Some(ctab) = reader.read(self, &lut) else {
            let mut b = b"ERROR: reading ".to_vec();
            b.extend_from_slice(&lut);
            b.push(b'\n');
            self.put_out(&b);
            return None;
        };
        let date = self.cur_time_stamp();
        let mut space = 2;
        if let Some(raw) = self.env.get("FS_DESIRED_BVEC_SPACE").map(<[u8]>::to_vec) {
            space = match clib::scan_d(&raw) {
                (1, Some(v)) => v,
                _ => {
                    self.note(
                        &FS_08,
                        "FS_DESIRED_BVEC_SPACE holds no number: the space is 0, where FreeSurfer reads memory it never wrote",
                    );
                    0
                }
            };
            if space != 1 && space != 2 {
                let mut b = b"ERROR: FS_DESIRED_BVEC_SPACE = ".to_vec();
                b.extend_from_slice(&raw);
                b.extend_from_slice(b", must be 1 or 2\n");
                self.put_out(&b);
                return None;
            }
        }
        Some(FsEnv {
            freesurfer_home: home,
            subjects_dir,
            user,
            cwd,
            sysname,
            machine,
            hostname,
            date,
            desired_bvec_space: space,
            ctab,
        })
    }

    /// `FSENVgetSUBJECTS_DIR`: its value, or `FSENVgetSUBJECTS_DIR: SUBJECTS_DIR not defined`
    /// on standard output and `None`.
    pub fn fsenv_get_subjects_dir(&mut self) -> Option<Vec<u8>> {
        let v = self.env.get("SUBJECTS_DIR").map(<[u8]>::to_vec);
        if v.is_none() {
            self.put_out(b"FSENVgetSUBJECTS_DIR: SUBJECTS_DIR not defined\n");
        }
        v
    }

    /// `FSENVsetSUBJECTS_DIR`: sets the run's `SUBJECTS_DIR`, overwriting it.
    pub fn fsenv_set_subjects_dir(&mut self, dir: &[u8]) {
        self.env.set(b"SUBJECTS_DIR", dir);
    }
}
