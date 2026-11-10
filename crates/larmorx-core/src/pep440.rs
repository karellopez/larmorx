// SPDX-License-Identifier: Apache-2.0
// Cargo's version in PEP 440's form, the one pip, PyPI and the wheels show: `0.1.0-rc.1` is
// `0.1.0rc1`, `0.2.0-alpha.3` is `0.2.0a3`, `0.2.0-beta.1` is `0.2.0b1` (maturin converts the
// wheel's version the same way; scripts/release/set_version.py writes the Cargo form). Shared by
// the build script, which sets `larmorx_core::VERSION`, and this crate's tests; so it holds plain
// items only (`include!`d by build.rs).

/// `X.Y.Z` or `X.Y.Z-alpha.N`, `-beta.N`, `-rc.N` in PEP 440's form; any other version is an
/// error (larmorx releases no other kind, and a silent conversion could disagree with maturin's).
pub fn cargo_to_pep440(version: &str) -> Result<String, String> {
    let refuse = || {
        format!(
            "larmorx-core: version {version:?} is not X.Y.Z, X.Y.Z-alpha.N, X.Y.Z-beta.N or \
             X.Y.Z-rc.N (scripts/release/set_version.py)"
        )
    };
    let number = |s: &str| -> Result<u64, String> {
        if s.is_empty()
            || !s.bytes().all(|b| b.is_ascii_digit())
            || (s.len() > 1 && s.starts_with('0'))
        {
            return Err(refuse());
        }
        s.parse::<u64>().map_err(|_| refuse())
    };
    let (release, pre) = match version.split_once('-') {
        Some((release, pre)) => (release, Some(pre)),
        None => (version, None),
    };
    let parts: Vec<&str> = release.split('.').collect();
    if parts.len() != 3 {
        return Err(refuse());
    }
    let mut out = String::new();
    for (i, part) in parts.iter().enumerate() {
        if i > 0 {
            out.push('.');
        }
        out.push_str(&number(part)?.to_string());
    }
    if let Some(pre) = pre {
        let (kind, n) = pre.split_once('.').ok_or_else(refuse)?;
        let tag = match kind {
            "alpha" => "a",
            "beta" => "b",
            "rc" => "rc",
            _ => return Err(refuse()),
        };
        out.push_str(tag);
        out.push_str(&number(n)?.to_string());
    }
    Ok(out)
}
