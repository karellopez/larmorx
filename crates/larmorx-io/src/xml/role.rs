// SPDX-License-Identifier: Apache-2.0 AND MIT
// Translated from expat 2.0.1's xmlrole.c and xmlrole.h (Copyright (c) 1998, 1999 Thai Open Source
// Software Center Ltd; MIT, PROVENANCE.md): the prolog's state machine, which gives each token
// of the prolog its role, for the document entity (an internal subset; external subsets and
// conditional sections are never read by a parser that reads no external entities).

use super::tok::*;

pub(super) const ROLE_ERROR: i32 = -1;
pub(super) const ROLE_NONE: i32 = 0;
pub(super) const ROLE_XML_DECL: i32 = 1;
pub(super) const ROLE_INSTANCE_START: i32 = 2;
pub(super) const ROLE_DOCTYPE_NONE: i32 = 3;
pub(super) const ROLE_DOCTYPE_NAME: i32 = 4;
pub(super) const ROLE_DOCTYPE_SYSTEM_ID: i32 = 5;
pub(super) const ROLE_DOCTYPE_PUBLIC_ID: i32 = 6;
pub(super) const ROLE_DOCTYPE_INTERNAL_SUBSET: i32 = 7;
pub(super) const ROLE_DOCTYPE_CLOSE: i32 = 8;
pub(super) const ROLE_GENERAL_ENTITY_NAME: i32 = 9;
pub(super) const ROLE_PARAM_ENTITY_NAME: i32 = 10;
pub(super) const ROLE_ENTITY_NONE: i32 = 11;
pub(super) const ROLE_ENTITY_VALUE: i32 = 12;
pub(super) const ROLE_ENTITY_SYSTEM_ID: i32 = 13;
pub(super) const ROLE_ENTITY_PUBLIC_ID: i32 = 14;
pub(super) const ROLE_ENTITY_COMPLETE: i32 = 15;
pub(super) const ROLE_ENTITY_NOTATION_NAME: i32 = 16;
pub(super) const ROLE_NOTATION_NONE: i32 = 17;
pub(super) const ROLE_NOTATION_NAME: i32 = 18;
pub(super) const ROLE_NOTATION_SYSTEM_ID: i32 = 19;
pub(super) const ROLE_NOTATION_NO_SYSTEM_ID: i32 = 20;
pub(super) const ROLE_NOTATION_PUBLIC_ID: i32 = 21;
pub(super) const ROLE_ATTRIBUTE_NAME: i32 = 22;
pub(super) const ROLE_ATTRIBUTE_TYPE_CDATA: i32 = 23;
pub(super) const ROLE_ATTRIBUTE_TYPE_ID: i32 = 24;
pub(super) const ROLE_ATTRIBUTE_ENUM_VALUE: i32 = 31;
pub(super) const ROLE_ATTRIBUTE_NOTATION_VALUE: i32 = 32;
pub(super) const ROLE_ATTLIST_NONE: i32 = 33;
pub(super) const ROLE_ATTLIST_ELEMENT_NAME: i32 = 34;
pub(super) const ROLE_IMPLIED_ATTRIBUTE_VALUE: i32 = 35;
pub(super) const ROLE_REQUIRED_ATTRIBUTE_VALUE: i32 = 36;
pub(super) const ROLE_DEFAULT_ATTRIBUTE_VALUE: i32 = 37;
pub(super) const ROLE_FIXED_ATTRIBUTE_VALUE: i32 = 38;
pub(super) const ROLE_ELEMENT_NONE: i32 = 39;
pub(super) const ROLE_ELEMENT_NAME: i32 = 40;
pub(super) const ROLE_CONTENT_ANY: i32 = 41;
pub(super) const ROLE_CONTENT_EMPTY: i32 = 42;
pub(super) const ROLE_CONTENT_PCDATA: i32 = 43;
pub(super) const ROLE_GROUP_OPEN: i32 = 44;
pub(super) const ROLE_GROUP_CLOSE: i32 = 45;
pub(super) const ROLE_GROUP_CLOSE_REP: i32 = 46;
pub(super) const ROLE_GROUP_CLOSE_OPT: i32 = 47;
pub(super) const ROLE_GROUP_CLOSE_PLUS: i32 = 48;
pub(super) const ROLE_GROUP_CHOICE: i32 = 49;
pub(super) const ROLE_GROUP_SEQUENCE: i32 = 50;
pub(super) const ROLE_CONTENT_ELEMENT: i32 = 51;
pub(super) const ROLE_CONTENT_ELEMENT_REP: i32 = 52;
pub(super) const ROLE_CONTENT_ELEMENT_OPT: i32 = 53;
pub(super) const ROLE_CONTENT_ELEMENT_PLUS: i32 = 54;
pub(super) const ROLE_PI: i32 = 55;
pub(super) const ROLE_COMMENT: i32 = 56;
pub(super) const ROLE_PARAM_ENTITY_REF: i32 = 60;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum H {
    Prolog0,
    Prolog1,
    Prolog2,
    Doctype0,
    Doctype1,
    Doctype2,
    Doctype3,
    Doctype4,
    Doctype5,
    InternalSubset,
    Entity(u8),
    Notation(u8),
    Attlist(u8),
    Element(u8),
    DeclClose,
    Error,
}

/// `PROLOG_STATE` of the document entity.
pub(super) struct State {
    handler: H,
    /// The nesting of an element declaration's groups.
    pub level: u32,
    role_none: i32,
}

impl State {
    /// `XmlPrologStateInit`.
    pub(super) fn new() -> State {
        State {
            handler: H::Prolog0,
            level: 0,
            role_none: 0,
        }
    }

    /// `common`: a token no role takes.
    fn common(&mut self) -> i32 {
        self.handler = H::Error;
        ROLE_ERROR
    }

    /// `XmlTokenRole`: the role of the token `b[s..e]`.
    pub(super) fn role(&mut self, tok: i32, b: &[u8], s: usize, e: usize, enc: Enc) -> i32 {
        let m = enc.mbpc();
        let kw = |w: &[u8]| enc.name_matches_ascii(b, s, e, w);
        let decl_kw = |w: &[u8]| enc.name_matches_ascii(b, s + 2 * m, e, w);
        let pound_kw = |w: &[u8]| enc.name_matches_ascii(b, s + m, e, w);
        macro_rules! go {
            ($h:expr, $r:expr) => {{
                self.handler = $h;
                return $r;
            }};
        }
        match self.handler {
            H::Prolog0 | H::Prolog1 | H::Prolog2 => {
                let h = self.handler;
                match tok {
                    TOK_PROLOG_S => {
                        if h == H::Prolog0 {
                            self.handler = H::Prolog1;
                        }
                        return ROLE_NONE;
                    }
                    TOK_XML_DECL if h == H::Prolog0 => go!(H::Prolog1, ROLE_XML_DECL),
                    TOK_PI => {
                        if h == H::Prolog0 {
                            self.handler = H::Prolog1;
                        }
                        return ROLE_PI;
                    }
                    TOK_COMMENT => {
                        if h == H::Prolog0 {
                            self.handler = H::Prolog1;
                        }
                        return ROLE_COMMENT;
                    }
                    TOK_BOM if h != H::Prolog2 => return ROLE_NONE,
                    TOK_DECL_OPEN if h != H::Prolog2 => {
                        if decl_kw(b"DOCTYPE") {
                            go!(H::Doctype0, ROLE_DOCTYPE_NONE);
                        }
                    }
                    TOK_INSTANCE_START => go!(H::Error, ROLE_INSTANCE_START),
                    _ => {}
                }
                self.common()
            }
            H::Doctype0 => match tok {
                TOK_PROLOG_S => ROLE_DOCTYPE_NONE,
                TOK_NAME => go!(H::Doctype1, ROLE_DOCTYPE_NAME),
                _ => self.common(),
            },
            H::Doctype1 => match tok {
                TOK_PROLOG_S => ROLE_DOCTYPE_NONE,
                TOK_OPEN_BRACKET => go!(H::InternalSubset, ROLE_DOCTYPE_INTERNAL_SUBSET),
                TOK_DECL_CLOSE => go!(H::Prolog2, ROLE_DOCTYPE_CLOSE),
                TOK_NAME if kw(b"SYSTEM") => go!(H::Doctype3, ROLE_DOCTYPE_NONE),
                TOK_NAME if kw(b"PUBLIC") => go!(H::Doctype2, ROLE_DOCTYPE_NONE),
                _ => self.common(),
            },
            H::Doctype2 => match tok {
                TOK_PROLOG_S => ROLE_DOCTYPE_NONE,
                TOK_LITERAL => go!(H::Doctype3, ROLE_DOCTYPE_PUBLIC_ID),
                _ => self.common(),
            },
            H::Doctype3 => match tok {
                TOK_PROLOG_S => ROLE_DOCTYPE_NONE,
                TOK_LITERAL => go!(H::Doctype4, ROLE_DOCTYPE_SYSTEM_ID),
                _ => self.common(),
            },
            H::Doctype4 => match tok {
                TOK_PROLOG_S => ROLE_DOCTYPE_NONE,
                TOK_OPEN_BRACKET => go!(H::InternalSubset, ROLE_DOCTYPE_INTERNAL_SUBSET),
                TOK_DECL_CLOSE => go!(H::Prolog2, ROLE_DOCTYPE_CLOSE),
                _ => self.common(),
            },
            H::Doctype5 => match tok {
                TOK_PROLOG_S => ROLE_DOCTYPE_NONE,
                TOK_DECL_CLOSE => go!(H::Prolog2, ROLE_DOCTYPE_CLOSE),
                _ => self.common(),
            },
            H::InternalSubset => match tok {
                TOK_PROLOG_S => ROLE_NONE,
                TOK_DECL_OPEN if decl_kw(b"ENTITY") => go!(H::Entity(0), ROLE_ENTITY_NONE),
                TOK_DECL_OPEN if decl_kw(b"ATTLIST") => go!(H::Attlist(0), ROLE_ATTLIST_NONE),
                TOK_DECL_OPEN if decl_kw(b"ELEMENT") => go!(H::Element(0), ROLE_ELEMENT_NONE),
                TOK_DECL_OPEN if decl_kw(b"NOTATION") => go!(H::Notation(0), ROLE_NOTATION_NONE),
                TOK_PI => ROLE_PI,
                TOK_COMMENT => ROLE_COMMENT,
                TOK_PARAM_ENTITY_REF => ROLE_PARAM_ENTITY_REF,
                TOK_CLOSE_BRACKET => go!(H::Doctype5, ROLE_DOCTYPE_NONE),
                TOK_NONE => ROLE_NONE,
                _ => self.common(),
            },
            H::Entity(n) => self.entity(n, tok, &kw),
            H::Notation(n) => self.notation(n, tok, &kw),
            H::Attlist(n) => self.attlist(n, tok, &kw, &pound_kw),
            H::Element(n) => self.element(n, tok, &kw, &pound_kw),
            H::DeclClose => match tok {
                TOK_PROLOG_S => self.role_none,
                TOK_DECL_CLOSE => {
                    self.handler = H::InternalSubset; // setTopLevel: the document entity
                    self.role_none
                }
                _ => self.common(),
            },
            H::Error => ROLE_NONE,
        }
    }

    fn decl_close(&mut self, role_none: i32, r: i32) -> i32 {
        self.handler = H::DeclClose;
        self.role_none = role_none;
        r
    }

    fn entity(&mut self, n: u8, tok: i32, kw: &dyn Fn(&[u8]) -> bool) -> i32 {
        if tok == TOK_PROLOG_S {
            return ROLE_ENTITY_NONE;
        }
        let to = |s: &mut State, k: u8, r: i32| {
            s.handler = H::Entity(k);
            r
        };
        match (n, tok) {
            (0, TOK_PERCENT) => to(self, 1, ROLE_ENTITY_NONE),
            (0, TOK_NAME) => to(self, 2, ROLE_GENERAL_ENTITY_NAME),
            (1, TOK_NAME) => to(self, 7, ROLE_PARAM_ENTITY_NAME),
            (2, TOK_NAME) if kw(b"SYSTEM") => to(self, 4, ROLE_ENTITY_NONE),
            (2, TOK_NAME) if kw(b"PUBLIC") => to(self, 3, ROLE_ENTITY_NONE),
            (2, TOK_LITERAL) => self.decl_close(ROLE_ENTITY_NONE, ROLE_ENTITY_VALUE),
            (3, TOK_LITERAL) => to(self, 4, ROLE_ENTITY_PUBLIC_ID),
            (4, TOK_LITERAL) => to(self, 5, ROLE_ENTITY_SYSTEM_ID),
            (5, TOK_DECL_CLOSE) => {
                self.handler = H::InternalSubset;
                ROLE_ENTITY_COMPLETE
            }
            (5, TOK_NAME) if kw(b"NDATA") => to(self, 6, ROLE_ENTITY_NONE),
            (6, TOK_NAME) => self.decl_close(ROLE_ENTITY_NONE, ROLE_ENTITY_NOTATION_NAME),
            (7, TOK_NAME) if kw(b"SYSTEM") => to(self, 9, ROLE_ENTITY_NONE),
            (7, TOK_NAME) if kw(b"PUBLIC") => to(self, 8, ROLE_ENTITY_NONE),
            (7, TOK_LITERAL) => self.decl_close(ROLE_ENTITY_NONE, ROLE_ENTITY_VALUE),
            (8, TOK_LITERAL) => to(self, 9, ROLE_ENTITY_PUBLIC_ID),
            (9, TOK_LITERAL) => to(self, 10, ROLE_ENTITY_SYSTEM_ID),
            (10, TOK_DECL_CLOSE) => {
                self.handler = H::InternalSubset;
                ROLE_ENTITY_COMPLETE
            }
            _ => self.common(),
        }
    }

    fn notation(&mut self, n: u8, tok: i32, kw: &dyn Fn(&[u8]) -> bool) -> i32 {
        if tok == TOK_PROLOG_S {
            return ROLE_NOTATION_NONE;
        }
        let to = |s: &mut State, k: u8, r: i32| {
            s.handler = H::Notation(k);
            r
        };
        match (n, tok) {
            (0, TOK_NAME) => to(self, 1, ROLE_NOTATION_NAME),
            (1, TOK_NAME) if kw(b"SYSTEM") => to(self, 3, ROLE_NOTATION_NONE),
            (1, TOK_NAME) if kw(b"PUBLIC") => to(self, 2, ROLE_NOTATION_NONE),
            (2, TOK_LITERAL) => to(self, 4, ROLE_NOTATION_PUBLIC_ID),
            (3 | 4, TOK_LITERAL) => self.decl_close(ROLE_NOTATION_NONE, ROLE_NOTATION_SYSTEM_ID),
            (4, TOK_DECL_CLOSE) => {
                self.handler = H::InternalSubset;
                ROLE_NOTATION_NO_SYSTEM_ID
            }
            _ => self.common(),
        }
    }

    fn attlist(
        &mut self,
        n: u8,
        tok: i32,
        kw: &dyn Fn(&[u8]) -> bool,
        pound: &dyn Fn(&[u8]) -> bool,
    ) -> i32 {
        if tok == TOK_PROLOG_S {
            return ROLE_ATTLIST_NONE;
        }
        let to = |s: &mut State, k: u8, r: i32| {
            s.handler = H::Attlist(k);
            r
        };
        const TYPES: [&[u8]; 8] = [
            b"CDATA",
            b"ID",
            b"IDREF",
            b"IDREFS",
            b"ENTITY",
            b"ENTITIES",
            b"NMTOKEN",
            b"NMTOKENS",
        ];
        match (n, tok) {
            (0, TOK_NAME) => to(self, 1, ROLE_ATTLIST_ELEMENT_NAME),
            (1, TOK_DECL_CLOSE) => {
                self.handler = H::InternalSubset;
                ROLE_ATTLIST_NONE
            }
            (1, TOK_NAME) => to(self, 2, ROLE_ATTRIBUTE_NAME),
            (2, TOK_NAME) => {
                if let Some(i) = TYPES.iter().position(|t| kw(t)) {
                    return to(self, 8, ROLE_ATTRIBUTE_TYPE_CDATA + i as i32);
                }
                if kw(b"NOTATION") {
                    return to(self, 5, ROLE_ATTLIST_NONE);
                }
                self.common()
            }
            (2, TOK_OPEN_PAREN) => to(self, 3, ROLE_ATTLIST_NONE),
            (3, TOK_NMTOKEN | TOK_NAME) => to(self, 4, ROLE_ATTRIBUTE_ENUM_VALUE),
            (4, TOK_CLOSE_PAREN) => to(self, 8, ROLE_ATTLIST_NONE),
            (4, TOK_OR) => to(self, 3, ROLE_ATTLIST_NONE),
            (5, TOK_OPEN_PAREN) => to(self, 6, ROLE_ATTLIST_NONE),
            (6, TOK_NAME) => to(self, 7, ROLE_ATTRIBUTE_NOTATION_VALUE),
            (7, TOK_CLOSE_PAREN) => to(self, 8, ROLE_ATTLIST_NONE),
            (7, TOK_OR) => to(self, 6, ROLE_ATTLIST_NONE),
            (8, TOK_POUND_NAME) if pound(b"IMPLIED") => to(self, 1, ROLE_IMPLIED_ATTRIBUTE_VALUE),
            (8, TOK_POUND_NAME) if pound(b"REQUIRED") => to(self, 1, ROLE_REQUIRED_ATTRIBUTE_VALUE),
            (8, TOK_POUND_NAME) if pound(b"FIXED") => to(self, 9, ROLE_ATTLIST_NONE),
            (8, TOK_LITERAL) => to(self, 1, ROLE_DEFAULT_ATTRIBUTE_VALUE),
            (9, TOK_LITERAL) => to(self, 1, ROLE_FIXED_ATTRIBUTE_VALUE),
            _ => self.common(),
        }
    }

    fn element(
        &mut self,
        n: u8,
        tok: i32,
        kw: &dyn Fn(&[u8]) -> bool,
        pound: &dyn Fn(&[u8]) -> bool,
    ) -> i32 {
        if tok == TOK_PROLOG_S {
            return ROLE_ELEMENT_NONE;
        }
        let to = |s: &mut State, k: u8, r: i32| {
            s.handler = H::Element(k);
            r
        };
        let name_roles = |s: &mut State, tok: i32| -> Option<i32> {
            let r = match tok {
                TOK_NAME => ROLE_CONTENT_ELEMENT,
                TOK_NAME_QUESTION => ROLE_CONTENT_ELEMENT_OPT,
                TOK_NAME_ASTERISK => ROLE_CONTENT_ELEMENT_REP,
                TOK_NAME_PLUS => ROLE_CONTENT_ELEMENT_PLUS,
                _ => return None,
            };
            s.handler = H::Element(7);
            Some(r)
        };
        match (n, tok) {
            (0, TOK_NAME) => to(self, 1, ROLE_ELEMENT_NAME),
            (1, TOK_NAME) if kw(b"EMPTY") => self.decl_close(ROLE_ELEMENT_NONE, ROLE_CONTENT_EMPTY),
            (1, TOK_NAME) if kw(b"ANY") => self.decl_close(ROLE_ELEMENT_NONE, ROLE_CONTENT_ANY),
            (1, TOK_OPEN_PAREN) => {
                self.level = 1;
                to(self, 2, ROLE_GROUP_OPEN)
            }
            (2, TOK_POUND_NAME) if pound(b"PCDATA") => to(self, 3, ROLE_CONTENT_PCDATA),
            (2, TOK_OPEN_PAREN) => {
                self.level = 2;
                to(self, 6, ROLE_GROUP_OPEN)
            }
            (2 | 6, _) if name_roles(self, tok).is_some() => {
                name_roles(self, tok).expect("a name role")
            }
            (3, TOK_CLOSE_PAREN) => self.decl_close(ROLE_ELEMENT_NONE, ROLE_GROUP_CLOSE),
            (3, TOK_CLOSE_PAREN_ASTERISK) => {
                self.decl_close(ROLE_ELEMENT_NONE, ROLE_GROUP_CLOSE_REP)
            }
            (3, TOK_OR) => to(self, 4, ROLE_ELEMENT_NONE),
            (4, TOK_NAME) => to(self, 5, ROLE_CONTENT_ELEMENT),
            (5, TOK_CLOSE_PAREN_ASTERISK) => {
                self.decl_close(ROLE_ELEMENT_NONE, ROLE_GROUP_CLOSE_REP)
            }
            (5, TOK_OR) => to(self, 4, ROLE_ELEMENT_NONE),
            (6, TOK_OPEN_PAREN) => {
                self.level += 1;
                ROLE_GROUP_OPEN
            }
            (
                7,
                TOK_CLOSE_PAREN
                | TOK_CLOSE_PAREN_ASTERISK
                | TOK_CLOSE_PAREN_QUESTION
                | TOK_CLOSE_PAREN_PLUS,
            ) => {
                self.level -= 1;
                if self.level == 0 {
                    self.handler = H::DeclClose;
                    self.role_none = ROLE_ELEMENT_NONE;
                }
                match tok {
                    TOK_CLOSE_PAREN => ROLE_GROUP_CLOSE,
                    TOK_CLOSE_PAREN_ASTERISK => ROLE_GROUP_CLOSE_REP,
                    TOK_CLOSE_PAREN_QUESTION => ROLE_GROUP_CLOSE_OPT,
                    _ => ROLE_GROUP_CLOSE_PLUS,
                }
            }
            (7, TOK_COMMA) => to(self, 6, ROLE_GROUP_SEQUENCE),
            (7, TOK_OR) => to(self, 6, ROLE_GROUP_CHOICE),
            _ => self.common(),
        }
    }
}
