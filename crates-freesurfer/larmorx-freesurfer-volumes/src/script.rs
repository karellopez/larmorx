// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! What FreeSurfer 8.2.0's tcsh scripts `seg2cc`, `mergeseg` and `fscalc` do besides running the
//! ported programs: the shell they run in ([`Sh`]: the environment with its `setenv`s, tcsh's
//! current folder and directory stack, the two streams), the lines they print and log (`echo`,
//! `tee -a`, `>>`), the system programs they call (`date`, `uname -a`, `pwd`, `mkdir -p`, `rm`,
//! `cat`), the helpers of FreeSurfer's they call whose decision is all they need (`UpdateNeeded`,
//! `fsr-checkxopts`, `fs_temp_dir`, `fs_time`), and the ported programs run in this process with
//! the settings the script would give them ([`Sh::run`]).
//!
//! Measured on Ubuntu 22.04 (tcsh 6.21.00, GNU coreutils 8.32, GNU time 1.9, procps 3.3.17): the
//! official scripts and small test scripts, `validation/probes/fs-scripts/`.
//!
//! - **tcsh's current folder** starts as `PWD` when it names the folder the script starts in,
//!   else that folder's physical name; `pushd <dir>` makes it `<dir>` (relative to it) with `.`
//!   and `..` taken off as text, links kept; `pushd` and `popd` print the stack (the current
//!   folder, then the saved ones, each followed by a blank, the home folder as `~`); a program run
//!   from the script starts in the folder's physical name, with `PWD` set to tcsh's name.
//! - **A pipeline's status** (`cmd | tee -a log`, tcsh's `anyerror`): the status of the last
//!   command that failed, else 0.
//! - **`date`** in the C locale (`%a %b %e %H:%M:%S %Z %Y`; other locales' texts are not
//!   reproduced: their lines carry the C locale's form), **`uname -a`** as Ubuntu's coreutils
//!   prints it on Linux (kernel name, host, release, version, the machine three times,
//!   `GNU/Linux`).
//! - **`fs_time`** (FreeSurfer's front end of GNU `time`): the command's output, then `Command
//!   exited with non-zero status <n>` when it failed and the resource line `@#@FSTIME
//!   <%Y:%m:%d:%H:%M:%S> <program> N <arguments> e <elapsed> S <system> U <user> P <cpu>% M
//!   <peak kB> F <major faults> R <minor faults> W 0 c <involuntary> w <voluntary> I <blocks in> O
//!   <blocks out> L <load 1> <load 5> <load 15>`, then `@#@FSLOADPOST <date> <program> N
//!   <arguments> <loads>`. The figures of a program run in this process are the process's own
//!   (`/proc/self`), so they describe the port, not FreeSurfer. Only on Linux with
//!   `/usr/bin/time`, as FreeSurfer's (elsewhere the command alone); `FS_TIME_ALLOW=0` runs the
//!   command alone; `FSTIME_LOAD=0` leaves the loads out.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Instant;

use larmorx_core::defects::{Defect, FS_1277, FS_1278};
use larmorx_freesurfer_core::ctime;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::{errno_of, strerror};
use larmorx_freesurfer_core::run::{Environment, Settings};
use larmorx_freesurfer_core::system;

use crate::csh;

/// The end of a script: its exit status (its messages written).
pub type Flow<T = ()> = Result<T, u8>;

/// A log file (`$LF`): its name, and with `fixed` the file every line goes to (the default: the
/// name read from the folder the script started in); without, the name is read from tcsh's
/// current folder at each line, as FreeSurfer does.
#[derive(Clone, Debug)]
pub struct Log {
    pub name: String,
    pub fixed: Option<PathBuf>,
}

/// The shell a script runs in.
pub struct Sh<'a> {
    /// The script's name (`seg2cc`).
    pub name: &'static str,
    /// The script's own notices.
    pub g: Globals,
    pub threads: usize,
    /// The environment, with the script's `setenv`s.
    pub env: Environment,
    /// tcsh's current folder (absolute; links kept).
    pub cwd: PathBuf,
    /// The folders `pushd` saved.
    pub stack: Vec<PathBuf>,
    pub out: &'a mut dyn Write,
    pub err: &'a mut dyn Write,
}

/// Whether tcsh would read the text again as more than this one word or as another word: blanks,
/// new lines, patterns, braces, a leading `~` or `=`, or nothing at all (FS-1277).
pub fn read_again(w: &str) -> bool {
    w.is_empty()
        || w.contains([' ', '\t', '\n', '\r', '*', '?', '{'])
        || csh::has_pattern(w)
        || w.starts_with(['~', '='])
}

/// `name` made absolute from `cwd`, `.` and `..` taken off as text (tcsh's `pushd`).
pub fn canonical(cwd: &Path, name: &str) -> PathBuf {
    let full = if Path::new(name).is_absolute() || name.starts_with('/') {
        PathBuf::from(name)
    } else {
        cwd.join(name)
    };
    let mut parts: Vec<std::ffi::OsString> = Vec::new();
    let mut root = PathBuf::new();
    for c in full.components() {
        match c {
            std::path::Component::Prefix(_) | std::path::Component::RootDir => {
                root.push(c.as_os_str());
            }
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                parts.pop();
            }
            std::path::Component::Normal(s) => parts.push(s.to_owned()),
        }
    }
    let mut p = root;
    for s in parts {
        p.push(s);
    }
    p
}

/// GNU coreutils' quoting of a name in its messages in the C locale (`'name'`).
fn quoted(name: &str) -> String {
    format!("'{name}'")
}

impl<'a> Sh<'a> {
    /// The shell of a script started with `settings` (its folder: `settings.dir`, else the
    /// process's).
    pub fn new(
        name: &'static str,
        settings: &Settings,
        out: &'a mut dyn Write,
        err: &'a mut dyn Write,
    ) -> Sh<'a> {
        let start = match &settings.dir {
            Some(d) => d.clone(),
            None => std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
        };
        let physical = csh::physical(&start)
            .map(PathBuf::from)
            .unwrap_or_else(|| start.clone());
        // tcsh starts in PWD when it names the same folder
        let cwd = match settings.env.get("PWD").map(PathBuf::from) {
            Some(p)
                if p.is_absolute()
                    && csh::physical(&p).map(PathBuf::from).as_ref() == Some(&physical) =>
            {
                p
            }
            _ => physical,
        };
        Sh {
            name,
            g: Globals::new(settings.mode),
            threads: settings.threads,
            env: settings.env.clone(),
            cwd,
            stack: Vec::new(),
            out,
            err,
        }
    }

    pub fn reproduce(&self) -> bool {
        self.g.notices.reproduce()
    }

    /// A notice of the script's.
    pub fn notice(&mut self, defect: &'static Defect, detail: impl Into<String>) {
        self.g.notices.add(defect, detail);
    }

    /// An environment variable's value (`None`: not set).
    pub fn var(&self, name: &str) -> Option<String> {
        self.env.get(name).map(|v| v.to_string_lossy().into_owned())
    }

    /// `$NAME` of the environment: its value, or tcsh's `NAME: Undefined variable.` (status 1).
    pub fn var_or_stop(&mut self, name: &str) -> Flow<String> {
        match self.var(name) {
            Some(v) => Ok(v),
            None => self.stop(&format!("{name}: Undefined variable.")),
        }
    }

    /// tcsh's message on standard error, status 1.
    pub fn stop<T>(&mut self, line: &str) -> Flow<T> {
        let _ = writeln!(self.err, "{line}");
        Err(1)
    }

    /// FS-1277: a name tcsh would read again (blanks, patterns, ...): a notice; the name as given
    /// in both modes (tcsh's reading of it is not reproduced).
    pub fn check_name(&mut self, what: &str, value: &str) {
        if !read_again(value) {
            return;
        }
        let detail = if self.reproduce() {
            format!(
                "{}: tcsh's reading of {what} '{value}' (blanks, patterns, braces, ~, =N or an empty word) is not reproduced: the name as given, in both modes",
                self.name
            )
        } else {
            format!(
                "{}: {what} '{value}' read as given (FreeSurfer's tcsh reads it again as shell words)",
                self.name
            )
        };
        self.notice(&FS_1277, detail);
    }

    /// The physical name of tcsh's current folder (`/bin/pwd`).
    pub fn pwd(&self) -> String {
        csh::physical(&self.cwd).unwrap_or_else(|| self.cwd.to_string_lossy().into_owned())
    }

    /// `name` from tcsh's current folder.
    pub fn path(&self, name: &str) -> PathBuf {
        if Path::new(name).is_absolute() || name.starts_with('/') {
            PathBuf::from(name)
        } else {
            self.cwd.join(name)
        }
    }

    /// `-e name`.
    pub fn exists(&self, name: &str) -> bool {
        !name.is_empty() && std::fs::metadata(self.path(name)).is_ok()
    }

    /// `-dw name`: a folder the user may write into.
    pub fn dir_writable(&self, name: &str) -> bool {
        let p = self.path(name);
        match std::fs::metadata(&p) {
            Ok(m) if m.is_dir() => {
                larmorx_freesurfer_core::utils::directory_is_writable(None, &p.to_string_lossy())
            }
            _ => false,
        }
    }

    /// The bytes tcsh's `echo` prints for `words` (FS-1278: by default the words as written,
    /// joined by blanks, and a new line).
    pub fn echo_bytes(&mut self, words: &[String]) -> Vec<u8> {
        let mut plain = words.join(" ").into_bytes();
        plain.push(b'\n');
        // tcsh's reading is a Unix shell's (FreeSurfer runs there): elsewhere the lines as written
        if !cfg!(unix) {
            return plain;
        }
        let tcsh = csh::echo(words);
        if tcsh != plain {
            let line = String::from_utf8_lossy(&plain).trim_end().to_owned();
            if self.reproduce() {
                self.notice(
                    &FS_1278,
                    format!(
                        "{}: the line '{line}' printed through tcsh's echo, as in FreeSurfer",
                        self.name
                    ),
                );
                return tcsh;
            }
            self.notice(
                &FS_1278,
                format!(
                    "{}: the line '{line}' printed as written (FreeSurfer's echo reads -n and backslashes in it)",
                    self.name
                ),
            );
        }
        plain
    }

    /// `echo words`.
    pub fn echo(&mut self, words: &[String]) {
        let b = self.echo_bytes(words);
        let _ = self.out.write_all(&b);
    }

    /// `echo "line"` (one word).
    pub fn say(&mut self, line: &str) {
        self.echo(&[line.to_owned()]);
    }

    /// The file a log line goes to now.
    fn log_path(&self, lf: &Log) -> Option<PathBuf> {
        if lf.name == "/dev/null" {
            return None;
        }
        Some(lf.fixed.clone().unwrap_or_else(|| self.path(&lf.name)))
    }

    /// `... | tee -a $LF` for `text` (GNU tee): standard output and the log; a log that cannot be
    /// opened: `tee: <name>: <reason>` and status 1 (standard output still written).
    pub fn tee(&mut self, lf: &Log, text: &[u8]) -> u8 {
        let _ = self.out.write_all(text);
        match self.log_path(lf) {
            None => 0,
            Some(p) => match append_to(&p, text) {
                Ok(()) => 0,
                Err(e) => {
                    let _ = writeln!(self.err, "tee: {}: {}", lf.name, reason(&e));
                    1
                }
            },
        }
    }

    /// `echo words | tee -a $LF`.
    pub fn tee_echo(&mut self, lf: &Log, words: &[String]) -> u8 {
        let b = self.echo_bytes(words);
        self.tee(lf, &b)
    }

    /// `echo "line" | tee -a $LF`.
    pub fn tee_say(&mut self, lf: &Log, line: &str) -> u8 {
        self.tee_echo(lf, &[line.to_owned()])
    }

    /// `echo "line" >> $LF`: tcsh's `<name>: <reason>.` and the end of the script when the log
    /// cannot be opened.
    pub fn append_say(&mut self, lf: &Log, line: &str) -> Flow {
        let b = self.echo_bytes(&[line.to_owned()]);
        match self.log_path(lf) {
            None => Ok(()),
            Some(p) => match append_to(&p, &b) {
                Ok(()) => Ok(()),
                Err(e) => {
                    let r = reason(&e);
                    self.stop(&format!("{}: {r}.", lf.name))
                }
            },
        }
    }

    /// `rm -f name` (GNU rm): nothing for a missing file; a folder: `rm: cannot remove 'name': Is
    /// a directory`.
    pub fn rm_f(&mut self, name: &str) {
        if name == "/dev/null" {
            return;
        }
        let p = self.path(name);
        match std::fs::symlink_metadata(&p) {
            Err(_) => {}
            Ok(m) if m.is_dir() => {
                let _ = writeln!(
                    self.err,
                    "rm: cannot remove {}: Is a directory",
                    quoted(name)
                );
            }
            Ok(_) => {
                if let Err(e) = std::fs::remove_file(&p) {
                    let _ = writeln!(
                        self.err,
                        "rm: cannot remove {}: {}",
                        quoted(name),
                        reason(&e)
                    );
                }
            }
        }
    }

    /// `rm name` (GNU rm, no option): a missing file `rm: cannot remove 'name': No such file or
    /// directory`.
    pub fn rm(&mut self, name: &str) {
        let p = self.path(name);
        match std::fs::symlink_metadata(&p) {
            Err(e) => {
                let _ = writeln!(
                    self.err,
                    "rm: cannot remove {}: {}",
                    quoted(name),
                    reason(&e)
                );
            }
            Ok(m) if m.is_dir() => {
                let _ = writeln!(
                    self.err,
                    "rm: cannot remove {}: Is a directory",
                    quoted(name)
                );
            }
            Ok(_) => {
                if let Err(e) = std::fs::remove_file(&p) {
                    let _ = writeln!(
                        self.err,
                        "rm: cannot remove {}: {}",
                        quoted(name),
                        reason(&e)
                    );
                }
            }
        }
    }

    /// `rm -rf name`.
    pub fn rm_rf(&mut self, name: &str) {
        let p = self.path(name);
        match std::fs::symlink_metadata(&p) {
            Err(_) => {}
            Ok(m) if m.is_dir() => {
                if let Err(e) = std::fs::remove_dir_all(&p) {
                    let _ = writeln!(
                        self.err,
                        "rm: cannot remove {}: {}",
                        quoted(name),
                        reason(&e)
                    );
                }
            }
            Ok(_) => {
                let _ = std::fs::remove_file(&p);
            }
        }
    }

    /// `mkdir -p name` (GNU mkdir): its message and status 1 when a part is not a folder or
    /// cannot be made.
    pub fn mkdir_p(&mut self, name: &str) -> u8 {
        let p = self.path(name);
        if std::fs::metadata(&p).is_ok_and(|m| m.is_dir()) {
            return 0;
        }
        // the first part that is missing or not a folder
        let mut at = PathBuf::new();
        for c in p.components() {
            at.push(c.as_os_str());
            match std::fs::metadata(&at) {
                Ok(m) if m.is_dir() => continue,
                Ok(_) => {
                    let shown = shown_part(name, &p, &at);
                    let _ = writeln!(
                        self.err,
                        "mkdir: cannot create directory {}: Not a directory",
                        quoted(&shown)
                    );
                    return 1;
                }
                Err(_) => {}
            }
            if let Err(e) = std::fs::create_dir(&at)
                && !std::fs::metadata(&at).is_ok_and(|m| m.is_dir())
            {
                let shown = shown_part(name, &p, &at);
                let r = if std::fs::symlink_metadata(&at).is_ok() {
                    "File exists".to_owned()
                } else {
                    reason(&e)
                };
                let _ = writeln!(
                    self.err,
                    "mkdir: cannot create directory {}: {r}",
                    quoted(&shown)
                );
                return 1;
            }
        }
        0
    }

    /// `cat file` into a buffer (GNU cat): its message `cat: <name>: <reason>` and status 1.
    pub fn cat(&mut self, name: &str) -> (u8, Vec<u8>) {
        let p = self.path(name);
        match std::fs::read(&p) {
            Ok(b) => (0, b),
            Err(e) => {
                let r = if std::fs::metadata(&p).is_ok_and(|m| m.is_dir()) {
                    "Is a directory".to_owned()
                } else {
                    reason(&e)
                };
                let _ = writeln!(self.err, "cat: {name}: {r}");
                (1, Vec::new())
            }
        }
    }

    /// `date` (C locale).
    pub fn date(&self) -> String {
        ctime::date_c(&self.env, ctime::now())
    }

    /// `date '+%Y:%m:%d:%H:%M:%S'` (fs_time's stamp).
    pub fn date_stamp(&self) -> String {
        let tm = ctime::localtime(&self.env, ctime::now());
        format!(
            "{}:{:02}:{:02}:{:02}:{:02}:{:02}",
            tm.year + 1900,
            tm.mon + 1,
            tm.mday,
            tm.hour,
            tm.min,
            tm.sec
        )
    }

    /// `uname -a`.
    pub fn uname_a(&self) -> String {
        uname_a(&self.env)
    }

    /// The directory stack's line (`dirs`): each folder and a blank, the home folder as `~`.
    fn stack_line(&self) -> String {
        let home = self.var("HOME").filter(|h| h.starts_with('/'));
        let show = |p: &Path| -> String {
            let s = p.to_string_lossy().into_owned();
            match &home {
                Some(h) if s == *h => "~".to_owned(),
                Some(h) if s.starts_with(&format!("{}/", h.trim_end_matches('/'))) && h != "/" => {
                    format!("~{}", &s[h.trim_end_matches('/').len()..])
                }
                _ => s,
            }
        };
        let mut line = String::new();
        line.push_str(&show(&self.cwd));
        line.push(' ');
        for p in self.stack.iter().rev() {
            line.push_str(&show(p));
            line.push(' ');
        }
        line.push('\n');
        line
    }

    /// `pushd dir` (`print`: the stack's line on standard output); tcsh's message and the end of
    /// the script when the folder cannot be entered.
    pub fn pushd(&mut self, dir: &str, print: bool) -> Flow {
        if let Err(msg) = csh::enter(&self.cwd, dir) {
            return self.stop(&msg);
        }
        let new = canonical(&self.cwd, dir);
        let old = std::mem::replace(&mut self.cwd, new);
        self.stack.push(old);
        if print {
            let l = self.stack_line();
            let _ = self.out.write_all(l.as_bytes());
        }
        Ok(())
    }

    /// `popd`.
    pub fn popd(&mut self, print: bool) -> Flow {
        match self.stack.pop() {
            None => self.stop("popd: Directory stack empty."),
            Some(p) => {
                self.cwd = p;
                if print {
                    let l = self.stack_line();
                    let _ = self.out.write_all(l.as_bytes());
                }
                Ok(())
            }
        }
    }

    /// The settings of a program the script runs: its environment with `PWD` tcsh's folder, the
    /// folder's physical name.
    fn child(&self) -> Settings {
        let mut env = self.env.clone();
        env.set("PWD", self.cwd.as_os_str());
        Settings {
            threads: self.threads,
            mode: self.g.notices.mode(),
            env,
            dir: Some(PathBuf::from(self.pwd())),
        }
    }

    /// A program run by name (the ported programs of this crate): its status and standard output
    /// (its standard error goes to the script's); a name that is none of them: tcsh's `<name>:
    /// Command not found.`, status 1.
    pub fn run(&mut self, prog: &str, args: &[String]) -> (u8, Vec<u8>) {
        let settings = self.child();
        let mut buf = Vec::new();
        let status = match crate::PROGRAMS.iter().find(|p| p.name == prog) {
            Some(p) => (p.run)(args, &settings, &mut buf, &mut *self.err),
            None => {
                let _ = writeln!(self.err, "{prog}: Command not found.");
                1
            }
        };
        let _ = self.err.flush();
        (status, buf)
    }

    /// `fs_time prog args...` (module documentation): the command's status and everything on
    /// standard output.
    pub fn fs_time(&mut self, prog: &str, args: &[String]) -> (u8, Vec<u8>) {
        // fs_time's setenvs reach the command
        if self.var("FS_TIME_ALLOW").is_none() {
            self.env.set("FS_TIME_ALLOW", "1");
        }
        if self.var("FSTIME_LOAD").is_none() {
            self.env.set("FSTIME_LOAD", "1");
        }
        let gnu_time = cfg!(target_os = "linux") && Path::new("/usr/bin/time").exists();
        if self.var("FS_TIME_ALLOW").as_deref() == Some("0") || !gnu_time {
            return self.run(prog, args);
        }
        let load = self.var("FSTIME_LOAD").as_deref() != Some("0");
        let loads = if load { load_averages() } else { None };
        let dt = self.date_stamp();
        let before = Usage::now();
        let t0 = Instant::now();
        let (status, mut out) = self.run(prog, args);
        let elapsed = t0.elapsed();
        let after = Usage::now();
        if status != 0 {
            out.extend_from_slice(
                format!("Command exited with non-zero status {status}\n").as_bytes(),
            );
        }
        let nargs = args.len();
        let (u, s) = (
            after.utime.saturating_sub(before.utime),
            after.stime.saturating_sub(before.stime),
        );
        // ticks of 1/100 s
        let hund = |t: u64| format!("{}.{:02}", t / 100, t % 100);
        let r_ms = elapsed.as_millis() as u64;
        let pct = if r_ms > 0 {
            format!("{}%", (u + s) * 10 * 100 / r_ms)
        } else {
            "?%".to_owned()
        };
        let upt = match &loads {
            Some(l) => format!("L {l}"),
            None => String::new(),
        };
        let line = format!(
            "@#@FSTIME  {dt} {prog} N {nargs} e {}.{:02} S {} U {} P {pct} M {} F {} R {} W 0 c {} w {} I {} O {} {upt}\n",
            elapsed.as_secs(),
            elapsed.subsec_millis() / 10,
            hund(s),
            hund(u),
            after.hwm_kb,
            after.majflt.saturating_sub(before.majflt),
            after.minflt.saturating_sub(before.minflt),
            after.nvcsw.saturating_sub(before.nvcsw),
            after.vcsw.saturating_sub(before.vcsw),
            after.read_blocks.saturating_sub(before.read_blocks),
            after.write_blocks.saturating_sub(before.write_blocks),
        );
        out.extend_from_slice(line.as_bytes());
        if load && let Some(l) = load_averages() {
            let dt2 = self.date_stamp();
            out.extend_from_slice(format!("@#@FSLOADPOST {dt2} {prog} N {nargs} {l}\n").as_bytes());
        }
        (status, out)
    }

    /// `UpdateNeeded target sources...` (FreeSurfer's csh script): its output (`1` when the
    /// target is missing, a source is missing, or a source is newer than the target, else `0`)
    /// and status (1 for a missing source).
    pub fn update_needed(&self, target: &str, sources: &[String]) -> (String, u8) {
        let tp = self.path(target);
        let Ok(tm) = std::fs::metadata(&tp) else {
            return ("1".to_owned(), 0);
        };
        let t_mod = tm.modified().ok();
        for s in sources {
            let Ok(sm) = std::fs::metadata(self.path(s)) else {
                return ("1".to_owned(), 1);
            };
            // coreutils' test -nt: newer modification time (nanoseconds)
            if let (Some(a), Some(b)) = (sm.modified().ok(), t_mod)
                && a > b
            {
                return ("1".to_owned(), 0);
            }
        }
        ("0".to_owned(), 0)
    }

    /// `fsr-checkxopts file` (FreeSurfer's tcsh script): its message (`ERROR: cannot find
    /// <file>`, `ERROR: multiple occurrences of a command in expert options file <file>`) on
    /// standard output and its status: the lines without `#`, their first words (blank lines'
    /// empty), compared with the same without repeats.
    pub fn checkxopts(&mut self, file: &str) -> u8 {
        if !self.exists(file) {
            self.say(&format!("ERROR: cannot find {file}"));
            return 1;
        }
        let text = std::fs::read(self.path(file)).unwrap_or_default();
        let mut firsts: Vec<Vec<u8>> = Vec::new();
        if !text.is_empty() {
            let body = text.strip_suffix(b"\n").unwrap_or(&text);
            for line in body.split(|&b| b == b'\n') {
                if line.contains(&b'#') {
                    continue;
                }
                let first = line
                    .split(|&b| b == b' ' || b == b'\t')
                    .find(|w| !w.is_empty())
                    .unwrap_or(&[]);
                firsts.push(first.to_vec());
            }
        }
        let n1 = firsts.len();
        firsts.sort();
        firsts.dedup();
        if n1 != firsts.len() {
            self.say(&format!(
                "ERROR: multiple occurrences of a command in expert options file {file}"
            ));
            return 1;
        }
        0
    }

    /// `fs_temp_dir --scratch --base <base>` (FreeSurfer's program): a new folder
    /// `<base>/tmp.XXXXXX` (mode 0700), printed; its message and status 1 when it cannot be made.
    pub fn fs_temp_dir(&mut self, base: &str) -> (u8, String) {
        let mut env = self.env.clone();
        env.set("FS_TMPDIR", base);
        let dir = PathBuf::from(self.pwd());
        let mut g = Globals::new(self.g.notices.mode());
        match larmorx_freesurfer_core::utils::make_temp_dir(
            &mut g,
            &env,
            Some(&dir),
            &mut *self.err,
        ) {
            Ok(d) if !d.is_empty() => (0, d),
            _ => (1, String::new()),
        }
    }

    /// The script's notices, at its end.
    pub fn finish(&mut self, status: u8) -> u8 {
        let _ = self.out.flush();
        self.g.notices.print(self.err);
        status
    }
}

/// `uname -a` (module documentation).
pub fn uname_a(env: &Environment) -> String {
    let u = system::uname(env);
    let (sys, node, rel, mach) = match &u {
        Some(u) => (
            u.sysname.as_str(),
            u.nodename.as_str(),
            u.release.as_str(),
            u.machine.as_str(),
        ),
        None => ("unknown", "unknown", "unknown", std::env::consts::ARCH),
    };
    let version = std::fs::read_to_string("/proc/sys/kernel/version")
        .map(|v| v.trim_end_matches('\n').to_owned())
        .unwrap_or_default();
    if sys == "Linux" {
        format!("{sys} {node} {rel} {version} {mach} {mach} {mach} GNU/Linux")
    } else {
        format!("{sys} {node} {rel} {version} {mach}")
    }
}

/// The part of `name` that `at` (a prefix of `full`, its absolute form) stands for, as GNU
/// mkdir names it: the name as given up to that part.
fn shown_part(name: &str, full: &Path, at: &Path) -> String {
    let rest = full
        .strip_prefix(at)
        .map(|r| r.components().count())
        .unwrap_or(0);
    let mut parts: Vec<&str> = name.split('/').collect();
    while parts.len() > 1 && parts.last() == Some(&"") {
        parts.pop();
    }
    let keep = parts.len().saturating_sub(rest);
    parts[..keep].join("/")
}

/// An I/O error's text as the C library gives it (Linux's `errno`).
pub fn reason(e: &std::io::Error) -> String {
    match errno_of(e) {
        Some(n) => strerror(n),
        None => e.to_string(),
    }
}

/// Appends `text` to `path` (created when missing).
fn append_to(path: &Path, text: &[u8]) -> std::io::Result<()> {
    if std::fs::metadata(path).is_ok_and(|m| m.is_dir()) {
        return Err(std::io::Error::from(std::io::ErrorKind::IsADirectory));
    }
    let mut f = std::fs::OpenOptions::new()
        .append(true)
        .create(true)
        .open(path)?;
    f.write_all(text)
}

/// The three load averages as procps' `uptime` prints them (`%.2f`, C locale), from
/// `/proc/loadavg`; `None` without it.
fn load_averages() -> Option<String> {
    let s = std::fs::read_to_string("/proc/loadavg").ok()?;
    let v: Vec<f64> = s
        .split_ascii_whitespace()
        .take(3)
        .filter_map(|w| w.parse().ok())
        .collect();
    (v.len() == 3).then(|| format!("{:.2} {:.2} {:.2}", v[0], v[1], v[2]))
}

/// The process's resource figures (`/proc/self`; zeros where they cannot be read).
#[derive(Default)]
struct Usage {
    utime: u64,
    stime: u64,
    minflt: u64,
    majflt: u64,
    hwm_kb: u64,
    vcsw: u64,
    nvcsw: u64,
    read_blocks: u64,
    write_blocks: u64,
}

impl Usage {
    fn now() -> Usage {
        let mut u = Usage::default();
        if let Ok(stat) = std::fs::read_to_string("/proc/self/stat")
            && let Some(close) = stat.rfind(')')
        {
            // the fields after the command's name, from field 3 (state)
            let f: Vec<&str> = stat[close + 1..].split_ascii_whitespace().collect();
            let at = |n: usize| f.get(n - 3).and_then(|v| v.parse().ok()).unwrap_or(0);
            u.minflt = at(10);
            u.majflt = at(12);
            u.utime = at(14);
            u.stime = at(15);
        }
        if let Ok(status) = std::fs::read_to_string("/proc/self/status") {
            for l in status.lines() {
                let num = |l: &str| -> u64 {
                    l.split_ascii_whitespace()
                        .nth(1)
                        .and_then(|v| v.parse().ok())
                        .unwrap_or(0)
                };
                if l.starts_with("VmHWM:") {
                    u.hwm_kb = num(l);
                } else if l.starts_with("voluntary_ctxt_switches:") {
                    u.vcsw = num(l);
                } else if l.starts_with("nonvoluntary_ctxt_switches:") {
                    u.nvcsw = num(l);
                }
            }
        }
        if let Ok(io) = std::fs::read_to_string("/proc/self/io") {
            for l in io.lines() {
                let num = |l: &str| -> u64 {
                    l.split_ascii_whitespace()
                        .nth(1)
                        .and_then(|v| v.parse().ok())
                        .unwrap_or(0)
                };
                if l.starts_with("read_bytes:") {
                    u.read_blocks = num(l) / 512;
                } else if l.starts_with("write_bytes:") {
                    u.write_blocks = num(l) / 512;
                }
            }
        }
        u
    }
}

/// `bc -l`'s `<seconds>/3600` (20 decimals, truncated) printed by coreutils' `printf %5.2f`
/// (the decimal read as an x87 `long double`, rounded to two places), then read back as a word
/// (the leading blanks dropped): the scripts' `...-Run-Time-Hours`.
pub fn run_time_hours(seconds: i64) -> String {
    let neg = seconds < 0;
    let s = seconds.unsigned_abs() as u128;
    // the value in units of 1e-20 hours, truncated as bc's scale 20
    let units = s * 100_000_000_000_000_000_000u128 / 3600;
    // printf rounds the long double nearest to it: ties of two places only when the hundredths'
    // rest is exactly half (5e17 units), decided by the long double's side of the decimal
    let hundredths = units / 1_000_000_000_000_000_000;
    let rest = units % 1_000_000_000_000_000_000;
    let half = 500_000_000_000_000_000;
    let up = match rest.cmp(&half) {
        std::cmp::Ordering::Greater => true,
        std::cmp::Ordering::Less => false,
        std::cmp::Ordering::Equal => long_double_above(units, 20),
    };
    let h = hundredths + u128::from(up);
    let text = format!("{}.{:02}", h / 100, h % 100);
    if neg && h > 0 {
        format!("-{text}")
    } else if neg {
        format!("-{text}")
    } else {
        text
    }
}

/// Whether the 64-bit-mantissa binary number nearest to `units * 10^-places` (round half to
/// even) is above it.
fn long_double_above(units: u128, places: u32) -> bool {
    // value = units / 10^places = m * 2^e with m of 64 bits: find e so that 2^63 <= units *
    // 2^-e / 10^places < 2^64, compare the rounded m * 2^e with the value exactly
    if units == 0 {
        return false;
    }
    let den = num_pow10(places);
    // scale = 2^k numerator or denominator
    let mut e: i32 = 0;
    // value * 2^-e in [2^63, 2^64)
    let ge = |num: &Big, den: &Big, bits: u32| -> bool { num.ge(&den.shl(bits)) };
    let mut num = Big::from(units);
    let mut d = den.clone();
    // bring value into [2^63, 2^64)
    loop {
        if !ge(&num, &d, 63) {
            num = num.shl(1);
            e -= 1;
        } else if ge(&num, &d, 64) {
            d = d.shl(1);
            e += 1;
        } else {
            break;
        }
    }
    let _ = e;
    // m = floor(num / d), remainder r
    let (m, r) = num.divmod_small(&d);
    let twice_r = r.shl(1);
    match twice_r.cmp(&d) {
        std::cmp::Ordering::Greater => true,
        std::cmp::Ordering::Less => false,
        std::cmp::Ordering::Equal => m & 1 == 1,
    }
}

fn num_pow10(p: u32) -> Big {
    let mut b = Big::from(1);
    for _ in 0..p {
        b = b.mul_small(10);
    }
    b
}

/// A small unsigned big integer (base 2^32 limbs, little end first) for [`long_double_above`].
#[derive(Clone, Debug, PartialEq, Eq)]
struct Big(Vec<u32>);

impl Big {
    fn from(v: u128) -> Big {
        let mut l = Vec::new();
        let mut v = v;
        while v > 0 {
            l.push(v as u32);
            v >>= 32;
        }
        Big(l)
    }
    fn trim(mut self) -> Big {
        while self.0.last() == Some(&0) {
            self.0.pop();
        }
        self
    }
    fn mul_small(&self, k: u32) -> Big {
        let mut carry = 0u64;
        let mut out = Vec::with_capacity(self.0.len() + 1);
        for &x in &self.0 {
            let t = u64::from(x) * u64::from(k) + carry;
            out.push(t as u32);
            carry = t >> 32;
        }
        if carry > 0 {
            out.push(carry as u32);
        }
        Big(out).trim()
    }
    fn shl(&self, bits: u32) -> Big {
        let words = (bits / 32) as usize;
        let b = bits % 32;
        let mut out = vec![0u32; words];
        let mut carry = 0u32;
        for &x in &self.0 {
            if b == 0 {
                out.push(x);
            } else {
                out.push((x << b) | carry);
                carry = x >> (32 - b);
            }
        }
        if carry > 0 {
            out.push(carry);
        }
        Big(out).trim()
    }
    fn cmp(&self, o: &Big) -> std::cmp::Ordering {
        let (a, b) = (self.clone().trim(), o.clone().trim());
        a.0.len()
            .cmp(&b.0.len())
            .then_with(|| a.0.iter().rev().cmp(b.0.iter().rev()))
    }
    fn ge(&self, o: &Big) -> bool {
        self.cmp(o) != std::cmp::Ordering::Less
    }
    fn sub(&self, o: &Big) -> Big {
        let mut out = Vec::with_capacity(self.0.len());
        let mut borrow = 0i64;
        for i in 0..self.0.len() {
            let mut t = i64::from(self.0[i]) - borrow - i64::from(*o.0.get(i).unwrap_or(&0));
            borrow = 0;
            if t < 0 {
                t += 1 << 32;
                borrow = 1;
            }
            out.push(t as u32);
        }
        Big(out).trim()
    }
    /// `(self / d, self % d)` for a quotient below 2^64 (long division by bits).
    fn divmod_small(&self, d: &Big) -> (u64, Big) {
        let mut q = 0u64;
        let mut r = self.clone();
        for bit in (0..64).rev() {
            let t = d.shl(bit);
            if r.ge(&t) {
                r = r.sub(&t);
                q |= 1 << bit;
            }
        }
        (q, r)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_names() {
        let c = Path::new("/a/b");
        assert_eq!(canonical(c, "x/../y"), PathBuf::from("/a/b/y"));
        assert_eq!(canonical(c, "../../.."), PathBuf::from("/"));
        assert_eq!(canonical(c, "/p/./q/"), PathBuf::from("/p/q"));
    }

    #[test]
    fn hours_as_printf_prints_them() {
        // 18 s: bc .00500000000000000000; the long double nearest to 0.005 is above it
        assert_eq!(run_time_hours(18), "0.01");
        assert_eq!(run_time_hours(0), "0.00");
        assert_eq!(run_time_hours(3600), "1.00");
        assert_eq!(run_time_hours(17), "0.00");
        assert_eq!(run_time_hours(54), "0.01");
        assert_eq!(run_time_hours(90), "0.03");
    }

    #[test]
    fn read_again_forms() {
        assert!(read_again("a b") && read_again("a*") && read_again("~x") && read_again(""));
        assert!(!read_again("/a/b-c.mgz") && !read_again("x["));
    }
}
