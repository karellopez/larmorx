// SPDX-License-Identifier: Apache-2.0
//! POSIX basic regular expressions with GNU grep's extensions, in the C locale (bytes), for
//! `extracttxt`'s search (`specs/flirt.md` §15.8: the first line matching the word as GNU grep
//! reads it). Written from POSIX's definition and GNU grep's manual: `.`, bracket expressions
//! (ranges, `^`, the character classes), `*`, `\{m,n\}`, `\(` `\)` with back-references `\1` to
//! `\9`, the anchors `^` and `$`; GNU's `\+`, `\?`, `\|`, `\w`, `\W`, `\s`, `\S`, `\b`, `\B`,
//! `\<`, `\>`, `` \` `` and `\'`. Only whether a line holds a match is asked, so the search
//! backtracks through every way of matching.

/// One element of a pattern.
#[derive(Clone, Debug)]
enum Node {
    Byte(u8),
    Set(Box<[bool; 256]>),
    Group(usize, Vec<Vec<Node>>),
    Backref(usize),
    Repeat(Box<Node>, usize, Option<usize>),
    LineStart,
    LineEnd,
    WordBoundary(bool),
    WordStart,
    WordEnd,
}

/// A compiled pattern: its alternatives.
#[derive(Clone, Debug)]
pub struct Bre {
    alts: Vec<Vec<Node>>,
    groups: usize,
}

fn word(c: u8) -> bool {
    c.is_ascii_alphanumeric() || c == b'_'
}

fn space(c: u8) -> bool {
    matches!(c, b' ' | b'\t' | b'\n' | b'\r' | 0x0b | 0x0c)
}

fn class(name: &str) -> Option<fn(u8) -> bool> {
    Some(match name {
        "alpha" => |c: u8| c.is_ascii_alphabetic(),
        "digit" => |c: u8| c.is_ascii_digit(),
        "alnum" => |c: u8| c.is_ascii_alphanumeric(),
        "upper" => |c: u8| c.is_ascii_uppercase(),
        "lower" => |c: u8| c.is_ascii_lowercase(),
        "space" => space,
        "blank" => |c: u8| c == b' ' || c == b'\t',
        "punct" => |c: u8| c.is_ascii_punctuation(),
        "print" => |c: u8| (0x20..0x7f).contains(&c),
        "graph" => |c: u8| (0x21..0x7f).contains(&c),
        "cntrl" => |c: u8| c < 0x20 || c == 0x7f,
        "xdigit" => |c: u8| c.is_ascii_hexdigit(),
        _ => return None,
    })
}

struct Parser<'a> {
    p: &'a [u8],
    i: usize,
    groups: usize,
}

impl Parser<'_> {
    fn bracket(&mut self) -> Result<Node, String> {
        let unmatched = || "Unmatched [, [^, [:, [., or [=".to_owned();
        let mut set = [false; 256];
        let negate = self.p.get(self.i) == Some(&b'^');
        if negate {
            self.i += 1;
        }
        let mut first = true;
        loop {
            let Some(&c) = self.p.get(self.i) else {
                return Err(unmatched());
            };
            if c == b']' && !first {
                self.i += 1;
                break;
            }
            first = false;
            let lo = if c == b'[' && matches!(self.p.get(self.i + 1), Some(b':' | b'.' | b'=')) {
                let kind = self.p[self.i + 1];
                let start = self.i + 2;
                let end = (start..self.p.len().saturating_sub(1))
                    .find(|&k| self.p[k] == kind && self.p[k + 1] == b']')
                    .ok_or_else(unmatched)?;
                let name = std::str::from_utf8(&self.p[start..end]).unwrap_or("");
                self.i = end + 2;
                if kind == b':' {
                    let f = class(name).ok_or_else(|| "Invalid character class name".to_owned())?;
                    for b in 0..=255u8 {
                        if f(b) {
                            set[b as usize] = true;
                        }
                    }
                    continue;
                }
                match name.as_bytes() {
                    [one] => *one,
                    _ => return Err("Invalid collation character".to_owned()),
                }
            } else {
                self.i += 1;
                c
            };
            if self.p.get(self.i) == Some(&b'-') && self.p.get(self.i + 1).is_some_and(|&n| n != b']')
            {
                let hi = self.p[self.i + 1];
                self.i += 2;
                if hi < lo {
                    return Err("Invalid range end".to_owned());
                }
                for b in lo..=hi {
                    set[b as usize] = true;
                }
            } else {
                set[lo as usize] = true;
            }
        }
        if negate {
            for v in &mut set {
                *v = !*v;
            }
            set[b'\n' as usize] = false;
        }
        Ok(Node::Set(Box::new(set)))
    }

    /// A sequence of alternatives up to `\)` (when `nested`) or the end.
    fn alternatives(&mut self, nested: bool) -> Result<Vec<Vec<Node>>, String> {
        let mut alts = Vec::new();
        let mut seq: Vec<Node> = Vec::new();
        let mut at_start = true;
        loop {
            let Some(&c) = self.p.get(self.i) else {
                if nested {
                    return Err("Unmatched ( or \\(".to_owned());
                }
                alts.push(seq);
                return Ok(alts);
            };
            self.i += 1;
            let node = match c {
                b'\\' => {
                    let Some(&e) = self.p.get(self.i) else {
                        return Err("Trailing backslash".to_owned());
                    };
                    self.i += 1;
                    match e {
                        b'(' => {
                            self.groups += 1;
                            let g = self.groups;
                            Node::Group(g, self.alternatives(true)?)
                        }
                        b')' => {
                            if !nested {
                                return Err("Unmatched ) or \\)".to_owned());
                            }
                            alts.push(seq);
                            return Ok(alts);
                        }
                        b'|' => {
                            alts.push(std::mem::take(&mut seq));
                            at_start = true;
                            continue;
                        }
                        b'{' | b'+' | b'?' if !at_start => {
                            let (lo, hi) = if e == b'{' {
                                self.interval()?
                            } else if e == b'+' {
                                (1, None)
                            } else {
                                (0, Some(1))
                            };
                            let last = seq.pop().ok_or("Invalid preceding regular expression")?;
                            seq.push(Node::Repeat(Box::new(last), lo, hi));
                            continue;
                        }
                        b'1'..=b'9' => {
                            let n = usize::from(e - b'0');
                            if n > self.groups {
                                return Err("Invalid back reference".to_owned());
                            }
                            Node::Backref(n)
                        }
                        b'w' | b'W' | b's' | b'S' => {
                            let f: fn(u8) -> bool = if e.eq_ignore_ascii_case(&b'w') {
                                word
                            } else {
                                space
                            };
                            let mut set = [false; 256];
                            for b in 0..=255u8 {
                                set[b as usize] = f(b) == e.is_ascii_lowercase();
                            }
                            Node::Set(Box::new(set))
                        }
                        b'b' => Node::WordBoundary(true),
                        b'B' => Node::WordBoundary(false),
                        b'<' => Node::WordStart,
                        b'>' => Node::WordEnd,
                        b'`' => Node::LineStart,
                        b'\'' => Node::LineEnd,
                        other => Node::Byte(other),
                    }
                }
                b'.' => {
                    let mut set = [true; 256];
                    set[b'\n' as usize] = false;
                    Node::Set(Box::new(set))
                }
                b'[' => self.bracket()?,
                b'*' if !at_start => {
                    let last = seq.pop().ok_or("Invalid preceding regular expression")?;
                    seq.push(Node::Repeat(Box::new(last), 0, None));
                    continue;
                }
                b'^' if at_start => Node::LineStart,
                b'$' if self.i == self.p.len()
                    || self.p[self.i..].starts_with(b"\\)")
                    || self.p[self.i..].starts_with(b"\\|") =>
                {
                    Node::LineEnd
                }
                other => Node::Byte(other),
            };
            at_start = matches!(node, Node::LineStart) && seq.is_empty();
            seq.push(node);
        }
    }

    fn interval(&mut self) -> Result<(usize, Option<usize>), String> {
        let bad = || "Invalid content of \\{\\}".to_owned();
        let num = |s: &mut Self| -> Option<usize> {
            let st = s.i;
            while s.p.get(s.i).is_some_and(u8::is_ascii_digit) {
                s.i += 1;
            }
            std::str::from_utf8(&s.p[st..s.i]).ok()?.parse().ok()
        };
        let lo = num(self);
        let hi = if self.p.get(self.i) == Some(&b',') {
            self.i += 1;
            num(self)
        } else {
            Some(lo.ok_or_else(bad)?)
        };
        if !self.p[self.i..].starts_with(b"\\}") {
            return Err("Unmatched \\{".to_owned());
        }
        self.i += 2;
        let lo = lo.unwrap_or(0);
        if hi.is_some_and(|h| h < lo) || lo > 0x7fff || hi.is_some_and(|h| h > 0x7fff) {
            return Err(bad());
        }
        Ok((lo, hi))
    }
}

/// The captures of one attempt: each group's span.
type Caps = Vec<Option<(usize, usize)>>;

struct Matcher<'a> {
    line: &'a [u8],
    steps: std::cell::Cell<u64>,
}

/// More backtracking steps than this: the search is given up (the caller refuses).
const BUDGET: u64 = 50_000_000;

impl Matcher<'_> {
    fn is_word(&self, pos: usize) -> bool {
        pos < self.line.len() && word(self.line[pos])
    }

    fn seq(
        &self,
        nodes: &[Node],
        pos: usize,
        caps: &mut Caps,
        k: &mut dyn FnMut(usize, &mut Caps) -> bool,
    ) -> bool {
        self.steps.set(self.steps.get() + 1);
        if self.steps.get() > BUDGET {
            return false;
        }
        let Some((first, rest)) = nodes.split_first() else {
            return k(pos, caps);
        };
        self.node(first, pos, caps, &mut |p, c| self.seq(rest, p, c, k))
    }

    fn node(
        &self,
        n: &Node,
        pos: usize,
        caps: &mut Caps,
        k: &mut dyn FnMut(usize, &mut Caps) -> bool,
    ) -> bool {
        let line = self.line;
        match n {
            Node::Byte(b) => pos < line.len() && line[pos] == *b && k(pos + 1, caps),
            Node::Set(s) => pos < line.len() && s[line[pos] as usize] && k(pos + 1, caps),
            Node::LineStart => pos == 0 && k(pos, caps),
            Node::LineEnd => pos == line.len() && k(pos, caps),
            Node::WordBoundary(want) => {
                let before = pos > 0 && self.is_word(pos - 1);
                ((before != self.is_word(pos)) == *want) && k(pos, caps)
            }
            Node::WordStart => {
                !(pos > 0 && self.is_word(pos - 1)) && self.is_word(pos) && k(pos, caps)
            }
            Node::WordEnd => pos > 0 && self.is_word(pos - 1) && !self.is_word(pos) && k(pos, caps),
            Node::Backref(g) => {
                let Some((s, e)) = caps[*g] else {
                    return false;
                };
                let len = e - s;
                pos + len <= line.len() && line[s..e] == line[pos..pos + len] && k(pos + len, caps)
            }
            Node::Group(g, alts) => alts.iter().any(|alt| {
                let saved = caps[*g];
                let ok = self.seq(alt, pos, caps, &mut |p, c| {
                    let inner = c[*g];
                    c[*g] = Some((pos, p));
                    let r = k(p, c);
                    c[*g] = inner;
                    r
                });
                caps[*g] = saved;
                ok
            }),
            Node::Repeat(inner, lo, hi) => self.repeat(inner, *lo, *hi, 0, pos, caps, k),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn repeat(
        &self,
        inner: &Node,
        lo: usize,
        hi: Option<usize>,
        count: usize,
        pos: usize,
        caps: &mut Caps,
        k: &mut dyn FnMut(usize, &mut Caps) -> bool,
    ) -> bool {
        if hi.is_none_or(|h| count < h)
            && self.node(inner, pos, caps, &mut |p, c| {
                (p != pos || count < lo) && self.repeat(inner, lo, hi, count + 1, p, c, k)
            })
        {
            return true;
        }
        count >= lo && k(pos, caps)
    }
}

impl Bre {
    /// Compiles `pattern`; `Err` with GNU grep's message for an invalid one.
    pub fn new(pattern: &[u8]) -> Result<Bre, String> {
        let mut p = Parser {
            p: pattern,
            i: 0,
            groups: 0,
        };
        let alts = p.alternatives(false)?;
        Ok(Bre {
            alts,
            groups: p.groups,
        })
    }

    /// Whether `line` holds a match; `None` when the search exceeds its budget.
    pub fn is_match(&self, line: &[u8]) -> Option<bool> {
        let m = Matcher {
            line,
            steps: std::cell::Cell::new(0),
        };
        for start in 0..=line.len() {
            for alt in &self.alts {
                let mut caps: Caps = vec![None; self.groups + 1];
                if m.seq(alt, start, &mut caps, &mut |_, _| true) {
                    return Some(true);
                }
            }
            if m.steps.get() > BUDGET {
                return None;
            }
        }
        Some(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn m(p: &str, l: &str) -> bool {
        Bre::new(p.as_bytes()).unwrap().is_match(l.as_bytes()).unwrap()
    }

    #[test]
    fn basic_regular_expressions() {
        assert!(m("e.4", "line 4 value 12"));
        assert!(!m("e.4", "line 1 value 3"));
        assert!(m("^line.[5-6]", "line 5 value 15"));
        assert!(!m("^line.[5-6]", "the line 5"));
        assert!(m("end$", "the end"));
        assert!(!m("end$", "the end."));
        assert!(m("^line.1.v", "line 1 value 3"));
        assert!(m("a*b", "b"));
        assert!(m("*a", "x*a"));
        assert!(m("\\(ab\\)\\1", "xxababyy"));
        assert!(!m("\\(ab\\)\\1", "xxabacyy"));
        assert!(m("a\\{2,3\\}", "caab"));
        assert!(!m("ca\\{2,3\\}b", "cab"));
        assert!(m("x\\|end", "the end"));
        assert!(m("[[:digit:]]\\+", "a12"));
        assert!(m("\\<val", "a value"));
        assert!(!m("\\<alue", "a value"));
        assert!(m("[^a-z ]", "abc 9"));
        assert!(!m("[^a-z ]", "abc d"));
        assert!(m("a.c", "a]c"));
        assert!(m("[]a]", "]"));
        assert!(m("a$b", "a$b"));
        assert!(m("a^b", "a^b"));
        assert!(Bre::new(b"[abc").is_err());
        assert!(Bre::new(b"\\(a").is_err());
        assert!(Bre::new(b"a\\{2").is_err());
    }
}
