// SPDX-License-Identifier: Apache-2.0
//! What the version stamps and the environment record read about the system (spec §13, §15):
//! the effective user's account name, the host name, the kernel's release, the system's name and
//! machine type, the working directory. Read without the C library: Linux's `/proc` and
//! `/etc/passwd`; elsewhere what the standard library gives (the user, the host name and the
//! release are then unknown).

/// The account name of the process's effective user (`getpwuid(geteuid())`), `None` when it
/// cannot be had. Linux: the effective user id from `/proc/self/status`, its name from
/// `/etc/passwd` (accounts known only to other name services are not found).
pub fn effective_user() -> Option<Vec<u8>> {
    let status = std::fs::read("/proc/self/status").ok()?;
    let line = status
        .split(|&c| c == b'\n')
        .find(|l| l.starts_with(b"Uid:"))?;
    let euid = line[4..]
        .split(|c| c.is_ascii_whitespace())
        .filter(|f| !f.is_empty())
        .nth(1)?;
    let passwd = std::fs::read("/etc/passwd").ok()?;
    passwd.split(|&c| c == b'\n').find_map(|l| {
        let f: Vec<&[u8]> = l.split(|&c| c == b':').collect();
        (f.len() >= 3 && f[2] == euid && !f[0].is_empty()).then(|| f[0].to_vec())
    })
}

fn proc_line(path: &str) -> Option<Vec<u8>> {
    let mut b = std::fs::read(path).ok()?;
    while b.last() == Some(&b'\n') {
        b.pop();
    }
    Some(b)
}

/// The host name (`uname`'s node name).
pub fn host_name() -> Option<Vec<u8>> {
    proc_line("/proc/sys/kernel/hostname")
}

/// The kernel's release (`uname`'s release).
pub fn kernel_release() -> Option<Vec<u8>> {
    proc_line("/proc/sys/kernel/osrelease")
}

/// The system's name (`uname`'s system name).
pub fn system_name() -> Vec<u8> {
    if let Some(n) = proc_line("/proc/sys/kernel/ostype") {
        return n;
    }
    match std::env::consts::OS {
        "macos" => b"Darwin".to_vec(),
        "linux" => b"Linux".to_vec(),
        "windows" => b"Windows_NT".to_vec(),
        other => other.as_bytes().to_vec(),
    }
}

/// The machine type (`uname`'s machine).
pub fn machine_type() -> Vec<u8> {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("macos", "aarch64") => b"arm64".to_vec(),
        (_, arch) => arch.as_bytes().to_vec(),
    }
}

/// The working directory, `None` when it cannot be read.
pub fn working_directory() -> Option<Vec<u8>> {
    std::env::current_dir()
        .ok()
        .map(|d| crate::env::os_bytes(d.as_os_str()))
}
