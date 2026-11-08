// SPDX-License-Identifier: Apache-2.0 AND PSF-2.0
//! `textwrap.wrap(text, width)` and `textwrap.fill` with the default options of CPython 3.12.16
//! (`expand_tabs`, `replace_whitespace`, `drop_whitespace`, `break_long_words` and
//! `break_on_hyphens` true; module documentation in [`super`]).
//!
//! The text is split into chunks as `TextWrapper.wordsep_re` splits it: runs of whitespace, em
//! dashes between words (`--` after a word character or punctuation and before a word
//! character), and words, which may also end after a hyphen between letters (`goof-` in
//! `goof-ball`). Characters outside ASCII count as letters when alphabetic and as word
//! characters when alphanumeric (Python's `\w` on `str`).

/// ASCII whitespace as `textwrap._whitespace`: tab, newline, vertical tab, form feed, carriage
/// return and space.
fn is_ws(c: char) -> bool {
    matches!(c, '\t' | '\n' | '\x0b' | '\x0c' | '\r' | ' ')
}

/// Python's `\w` on `str`.
fn is_word(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// `[^\d\W]`: a word character that is not a digit.
fn is_letter(c: char) -> bool {
    is_word(c) && !c.is_numeric()
}

/// `[\w!"'&.,?]`.
fn is_word_punct(c: char) -> bool {
    is_word(c) || matches!(c, '!' | '"' | '\'' | '&' | '.' | ',' | '?')
}

/// `-{2,}\w` at `i`: two or more hyphens, then a word character.
fn em_dash_then_word(t: &[char], i: usize) -> bool {
    let mut j = i;
    while j < t.len() && t[j] == '-' {
        j += 1;
    }
    j - i >= 2 && j < t.len() && is_word(t[j])
}

/// The chunks of `wordsep_re.split(text)` without the empty strings.
fn chunks(t: &[char]) -> Vec<String> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < t.len() {
        let start = i;
        let end = if is_ws(t[i]) {
            let mut j = i;
            while j < t.len() && is_ws(t[j]) {
                j += 1;
            }
            j
        } else if i > 0 && is_word_punct(t[i - 1]) && em_dash_then_word(t, i) {
            let mut j = i;
            while t[j] == '-' {
                j += 1;
            }
            j
        } else {
            // The shortest run of non-whitespace that ends a word: after a hyphen between
            // letters, before whitespace or the end, or before an em dash.
            let mut j = i + 1;
            loop {
                if j >= t.len() || is_ws(t[j]) {
                    break j;
                }
                if t[j] == '-' {
                    let behind2 = j >= 2 && is_letter(t[j - 2]) && is_letter(t[j - 1]);
                    let behind3 =
                        j >= 3 && is_letter(t[j - 3]) && t[j - 2] == '-' && is_letter(t[j - 1]);
                    let ahead = j + 1 < t.len()
                        && is_letter(t[j + 1])
                        && ((j + 2 < t.len() && is_letter(t[j + 2]))
                            || (j + 3 < t.len() && t[j + 2] == '-' && is_letter(t[j + 3])));
                    if (behind2 || behind3) && ahead {
                        break j + 1;
                    }
                    if is_word_punct(t[j - 1]) && em_dash_then_word(t, j) {
                        break j;
                    }
                }
                j += 1;
            }
        };
        out.push(t[start..end].iter().collect());
        i = end;
    }
    out
}

/// Expands tabs to the next multiple of 8 and turns every other whitespace character into a
/// space (`_munge_whitespace`).
fn munge(text: &str) -> Vec<char> {
    let mut out = Vec::new();
    let mut col = 0usize;
    for c in text.chars() {
        match c {
            '\t' => {
                let n = 8 - col % 8;
                out.extend(std::iter::repeat_n(' ', n));
                col += n;
            }
            '\n' | '\r' => {
                out.push(' ');
                col = 0;
            }
            c if is_ws(c) => {
                out.push(' ');
                col += 1;
            }
            c => {
                out.push(c);
                col += 1;
            }
        }
    }
    out
}

/// `textwrap.wrap(text, width, initial_indent=initial, subsequent_indent=subsequent)` with the
/// other options at their defaults. `width` must be positive (Python raises `ValueError`).
pub fn wrap_indented(text: &str, width: usize, initial: &str, subsequent: &str) -> Vec<String> {
    assert!(width > 0, "invalid width {width} (must be > 0)");
    let mut rev: Vec<String> = chunks(&munge(text));
    rev.reverse();
    let mut lines = Vec::new();
    while !rev.is_empty() {
        let indent = if lines.is_empty() {
            initial
        } else {
            subsequent
        };
        let indent_len = indent.chars().count();
        // `width - len(indent)` may be negative in Python; then nothing fits and long words
        // are broken into one character per line.
        let avail = width as isize - indent_len as isize;
        if !lines.is_empty() && rev.last().is_some_and(|c| c.trim_matches(is_ws).is_empty()) {
            rev.pop();
        }
        let mut cur: Vec<String> = Vec::new();
        let mut cur_len = 0isize;
        while let Some(c) = rev.last() {
            let l = c.chars().count() as isize;
            if cur_len + l <= avail {
                cur_len += l;
                cur.push(rev.pop().expect("checked"));
            } else {
                break;
            }
        }
        if let Some(c) = rev.last()
            && c.chars().count() as isize > avail
        {
            // _handle_long_word
            let space_left = if avail < 1 { 1 } else { avail - cur_len };
            let chunk: Vec<char> = c.chars().collect();
            let mut end = space_left.max(0) as usize;
            if chunk.len() > end {
                let hyphen = chunk[..end.min(chunk.len())]
                    .iter()
                    .rposition(|&ch| ch == '-');
                if let Some(h) = hyphen
                    && h > 0
                    && chunk[..h].iter().any(|&ch| ch != '-')
                {
                    end = h + 1;
                }
            }
            let end = end.min(chunk.len());
            cur.push(chunk[..end].iter().collect());
            let rest: String = chunk[end..].iter().collect();
            // An empty rest stays, as in Python, and is dropped at the next line's start
            // (`''.strip() == ''`).
            *rev.last_mut().expect("checked") = rest;
        }
        if cur.last().is_some_and(|c| c.trim_matches(is_ws).is_empty()) {
            cur.pop();
        }
        if !cur.is_empty() {
            lines.push(format!("{indent}{}", cur.concat()));
        }
    }
    lines
}

/// `textwrap.wrap(text, width)`.
pub fn wrap(text: &str, width: usize) -> Vec<String> {
    wrap_indented(text, width, "", "")
}

/// `textwrap.fill(text, width, initial_indent=indent, subsequent_indent=indent)`.
pub fn fill(text: &str, width: usize, indent: &str) -> String {
    wrap_indented(text, width, indent, indent).join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chunks_follow_wordsep_re() {
        let t: Vec<char> = "Hello there -- you goof-ball, use the -b option!"
            .chars()
            .collect();
        assert_eq!(
            chunks(&t),
            [
                "Hello", " ", "there", " ", "--", " ", "you", " ", "goof-", "ball,", " ", "use",
                " ", "the", " ", "-b", " ", "option!"
            ]
        );
        let t: Vec<char> = "from '--mask', when".chars().collect();
        assert_eq!(chunks(&t), ["from", " ", "'", "--", "mask',", " ", "when"]);
    }

    #[test]
    fn wraps_as_python() {
        assert_eq!(
            wrap(
                "Multi-echo dataset for analysis. A set of echo-specific files",
                30
            ),
            [
                "Multi-echo dataset for",
                "analysis. A set of echo-",
                "specific files"
            ]
        );
        assert_eq!(wrap("abcdefghij", 4), ["abcd", "efgh", "ij"]);
    }
}
