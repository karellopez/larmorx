// SPDX-License-Identifier: Apache-2.0
//! The environment table a program run reads (porting guide §9.1): filled once from the
//! process's environment by a command line, or given explicitly by a library caller; the library
//! never reads or changes the process environment itself.

/// A program run's environment variables, as bytes, in the order they were given.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Environment {
    vars: Vec<(Vec<u8>, Vec<u8>)>,
}

/// Whether two names are the same variable: exactly on Unix, without regard to ASCII case on
/// Windows (as the system matches them).
fn same_name(a: &[u8], b: &[u8]) -> bool {
    if cfg!(windows) {
        a.eq_ignore_ascii_case(b)
    } else {
        a == b
    }
}

impl Environment {
    /// An empty table.
    pub fn new() -> Environment {
        Environment::default()
    }

    /// The process's environment, read once (for a command line).
    pub fn from_process() -> Environment {
        let mut e = Environment::new();
        for (k, v) in std::env::vars_os() {
            e.set(&os_bytes(&k), &os_bytes(&v));
        }
        e
    }

    /// A table of the given pairs (a later pair wins over an earlier one of the same name).
    pub fn from_pairs<K: AsRef<[u8]>, V: AsRef<[u8]>>(
        pairs: impl IntoIterator<Item = (K, V)>,
    ) -> Environment {
        let mut e = Environment::new();
        for (k, v) in pairs {
            e.set(k.as_ref(), v.as_ref());
        }
        e
    }

    /// The value of `name`, `None` when it is not set (an empty value is set).
    pub fn get(&self, name: &str) -> Option<&[u8]> {
        self.get_bytes(name.as_bytes())
    }

    /// [`Environment::get`] for a name given as bytes.
    pub fn get_bytes(&self, name: &[u8]) -> Option<&[u8]> {
        self.vars
            .iter()
            .find(|(k, _)| same_name(k, name))
            .map(|(_, v)| v.as_slice())
    }

    /// Whether `name` is set (even to an empty value).
    pub fn is_set(&self, name: &str) -> bool {
        self.get(name).is_some()
    }

    /// The value of `name` when it is set and not empty.
    pub fn non_empty(&self, name: &str) -> Option<&[u8]> {
        self.get(name).filter(|v| !v.is_empty())
    }

    /// Sets `name` to `value`, overwriting it.
    pub fn set(&mut self, name: &[u8], value: &[u8]) {
        if let Some(slot) = self.vars.iter_mut().find(|(k, _)| same_name(k, name)) {
            slot.1 = value.to_vec();
        } else {
            self.vars.push((name.to_vec(), value.to_vec()));
        }
    }

    /// Removes `name`.
    pub fn remove(&mut self, name: &[u8]) {
        self.vars.retain(|(k, _)| !same_name(k, name));
    }

    /// The pairs, in order.
    pub fn iter(&self) -> impl Iterator<Item = (&[u8], &[u8])> {
        self.vars.iter().map(|(k, v)| (k.as_slice(), v.as_slice()))
    }
}

/// An OS string's bytes (Unix: as they are; elsewhere: its UTF-8 form, lossily).
pub fn os_bytes(s: &std::ffi::OsStr) -> Vec<u8> {
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        s.as_bytes().to_vec()
    }
    #[cfg(not(unix))]
    {
        s.to_string_lossy().into_owned().into_bytes()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_get_remove() {
        let mut e = Environment::from_pairs([("A", "1"), ("B", ""), ("A", "2")]);
        assert_eq!(e.get("A"), Some(&b"2"[..]));
        assert_eq!(e.get("B"), Some(&b""[..]));
        assert!(e.is_set("B"));
        assert_eq!(e.non_empty("B"), None);
        assert_eq!(e.get("C"), None);
        e.remove(b"A");
        assert_eq!(e.get("A"), None);
        assert_eq!(e.iter().count(), 1);
    }
}
