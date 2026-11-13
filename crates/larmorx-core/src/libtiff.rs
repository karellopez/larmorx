// SPDX-License-Identifier: Apache-2.0 AND libtiff
//! libtiff 3.6.1's error and warning reporting (Sam Leffler, Silicon Graphics; libtiff's
//! notice, LICENSES/libtiff.txt): `TIFFError` and `TIFFWarning` pass the message to the
//! library-wide handler, if one is set (`TIFFSetErrorHandler`, `TIFFSetWarningHandler` return
//! the one they replace); the default Unix handlers write, to standard error, the module and
//! `": "` (when there is a module), `"Warning, "` for a warning, the message, and `".\n"`.
//!
//! The message is the C's `vfprintf` of its format and arguments: the caller formats it
//! (`crate::cfmt` has C's conversions).

use crate::cfmt::Streams;

/// A handler: the default Unix one, none (`TIFFSet...Handler(NULL)`: nothing written), or the
/// caller's own (module, message, streams).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Handler {
    #[default]
    Unix,
    None,
    Custom(fn(Option<&str>, &str, &mut dyn Streams)),
}

/// The two library-wide handlers (`_TIFFerrorHandler`, `_TIFFwarningHandler`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct TiffMessages {
    pub error: Handler,
    pub warning: Handler,
}

impl TiffMessages {
    /// `TIFFSetErrorHandler`: the handler it replaces.
    pub fn set_error_handler(&mut self, handler: Handler) -> Handler {
        std::mem::replace(&mut self.error, handler)
    }

    /// `TIFFSetWarningHandler`: the handler it replaces.
    pub fn set_warning_handler(&mut self, handler: Handler) -> Handler {
        std::mem::replace(&mut self.warning, handler)
    }

    /// `TIFFError(module, ...)` with the formatted `message`.
    pub fn error(&self, module: Option<&str>, message: &str, streams: &mut dyn Streams) {
        match self.error {
            Handler::Unix => unix(module, "", message, streams),
            Handler::None => {}
            Handler::Custom(f) => f(module, message, streams),
        }
    }

    /// `TIFFWarning(module, ...)` with the formatted `message`.
    pub fn warning(&self, module: Option<&str>, message: &str, streams: &mut dyn Streams) {
        match self.warning {
            Handler::Unix => unix(module, "Warning, ", message, streams),
            Handler::None => {}
            Handler::Custom(f) => f(module, message, streams),
        }
    }
}

/// `unixErrorHandler`, `unixWarningHandler`.
fn unix(module: Option<&str>, prefix: &str, message: &str, streams: &mut dyn Streams) {
    if let Some(m) = module {
        streams.err(&format!("{m}: "));
    }
    if !prefix.is_empty() {
        streams.err(prefix);
    }
    streams.err(message);
    streams.err(".\n");
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cfmt::{Captured, Stream};

    #[test]
    fn unix_handlers_and_replacing_them() {
        let mut t = TiffMessages::default();
        let mut c = Captured::default();
        t.error(
            Some("Fax3Decode"),
            "a.tif: Bad code word at scanline 7 (x 12)",
            &mut c,
        );
        t.warning(
            None,
            "unknown field with tag 50000 (0xc350) ignored",
            &mut c,
        );
        t.error(None, "", &mut c);
        assert_eq!(
            c.text(Stream::Err),
            "Fax3Decode: a.tif: Bad code word at scanline 7 (x 12).\nWarning, unknown field with \
             tag 50000 (0xc350) ignored.\n.\n"
        );
        assert_eq!(t.set_error_handler(Handler::None), Handler::Unix);
        fn mine(m: Option<&str>, s: &str, st: &mut dyn Streams) {
            st.out(&format!("[{}|{s}]", m.unwrap_or("-")));
        }
        assert_eq!(t.set_warning_handler(Handler::Custom(mine)), Handler::Unix);
        let mut c = Captured::default();
        t.error(Some("m"), "x", &mut c);
        t.warning(Some("m"), "y", &mut c);
        assert_eq!(c.text(Stream::Err), "");
        assert_eq!(c.text(Stream::Out), "[m|y]");
    }
}
