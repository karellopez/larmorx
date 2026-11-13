// SPDX-License-Identifier: Apache-2.0
//! The same script as `shim.c`'s `fdp_run`, run on larmorx-io's port, with the same dump format;
//! and the probe-side dialect of FreeSurfer 8.2.0's copy (its two changes: a message's text, the
//! 4-byte swap in pairs), which a FreeSurfer replica supplies itself.

use larmorx_core::cfmt::{Captured, Stream};
use larmorx_core::defects::{Notices, UpstreamDefects};
use larmorx_io::dicom::cond::{self, Condition, DCM_ELEMENTLENGTHERROR};
use larmorx_io::dicom::{self, CondStack, Data, DcmObject, Dialect};
use std::path::Path;

/// FreeSurfer 8.2.0's copy of CTN 3.0.6, as the probe models it.
pub struct FsDialect;

const FS_ELEMENT_LENGTH_ERROR: &[u8] = b"DCM Data Element (%04x %04x) longer (%ld) than remaining length (%ld) of                                    data in stream or file in %s";

impl Dialect for FsDialect {
    fn message(&self, c: Condition) -> Option<&'static [u8]> {
        if c == DCM_ELEMENTLENGTHERROR {
            Some(FS_ELEMENT_LENGTH_ERROR)
        } else {
            cond::ctn_message(c)
        }
    }
    fn swap_4byte_in_pairs(&self) -> bool {
        true
    }
}

fn put_u32(b: &mut Vec<u8>, v: u32) {
    b.extend_from_slice(&v.to_le_bytes());
}
fn put_u64(b: &mut Vec<u8>, v: u64) {
    b.extend_from_slice(&v.to_le_bytes());
}
fn put_s(b: &mut Vec<u8>, s: &[u8]) {
    put_u32(b, s.len() as u32);
    b.extend_from_slice(s);
}

fn dump_object(b: &mut Vec<u8>, o: Option<&DcmObject>) {
    b.push(b'J');
    let Some(o) = o else {
        b.push(b'0');
        return;
    };
    b.push(b'1');
    put_s(b, o.key_type.as_bytes());
    put_u32(b, o.object_type);
    put_u32(b, o.access_method);
    put_u32(b, o.delete_flag as u32);
    put_u32(b, o.group_length_flag as u32);
    put_u64(b, o.object_size);
    put_u64(b, o.offset);
    put_u64(b, o.pixel_size);
    put_u64(b, o.pixel_offset);
    put_u32(b, o.pixel_bits_allocated as u32);
    put_u32(b, o.pixel_representation as u32);
    put_u32(b, o.fd_open as u32);
    put_s(b, &o.file_name);
    put_u32(b, o.preamble_flag as u32);
    b.extend_from_slice(&o.preamble);
    put_u64(b, o.data_options);
    put_u64(b, o.meta_header_length);
    put_u32(b, o.long_vr_attributes as u32);
    put_s(b, &o.waveform_data_vr);
    put_u32(b, o.groups.len() as u32);
    for g in &o.groups {
        b.push(b'G');
        put_u32(b, g.group as u32);
        put_u64(b, g.base_length);
        put_u32(b, g.long_vr_attributes as u32);
        put_u32(b, g.elements.len() as u32);
        for e in &g.elements {
            b.push(b'E');
            put_u32(b, e.tag);
            put_u32(b, e.representation as u32);
            put_u64(b, e.multiplicity);
            put_u32(b, e.length);
            put_s(b, e.description.as_bytes());
            put_u32(b, e.byte_order as u32);
            put_u64(b, e.data_offset as u64);
            put_u64(b, e.current_offset as u64);
            put_u64(b, e.allocated_data_length);
            put_u64(b, e.original_data_length);
            put_u64(b, e.padded_data_length);
            put_u32(b, e.fragment_flag as u32);
            match &e.data {
                Data::Sequence(items) => {
                    b.push(b'Q');
                    put_u32(b, items.len() as u32);
                    for it in items {
                        dump_object(b, Some(it));
                    }
                }
                Data::OnDisk => b.push(b'Z'),
                Data::Bytes(v) => {
                    b.push(b'D');
                    put_u32(b, v.len() as u32);
                    b.extend_from_slice(v);
                }
            }
        }
    }
}

/// The port's run: dump, standard output, standard error, the notices' IDs.
pub struct PortRun {
    pub dump: Vec<u8>,
    pub out: Vec<u8>,
    pub err: Vec<u8>,
    pub notices: Vec<&'static str>,
    pub unsupported: bool,
}

pub fn run(file: &Path, ops: &[i64], dialect: &dyn Dialect) -> PortRun {
    let mut b = Vec::new();
    let mut stack = CondStack::new();
    let mut out = Captured::default();
    let mut notices = Notices::new(UpstreamDefects::Reproduce);
    let mut object: Option<DcmObject> = None;
    let mut unsupported = false;
    for pair in ops.chunks_exact(2) {
        let (op, arg) = (pair[0], pair[1]);
        match op {
            1 => {
                let c = match dicom::open_file(
                    file,
                    arg as u64,
                    &mut object,
                    dialect,
                    &mut stack,
                    &mut out,
                    &mut notices,
                ) {
                    Ok(c) => c,
                    Err(_) => {
                        unsupported = true;
                        0
                    }
                };
                b.push(b'O');
                put_u64(&mut b, c);
                dump_object(&mut b, object.as_ref());
            }
            2 => {
                let c = dicom::close_object(&mut object, dialect, &mut stack, &mut out, &mut notices);
                b.push(b'X');
                put_u64(&mut b, c);
                b.push(if object.is_none() { b'0' } else { b'1' });
            }
            3 => {
                b.push(b'S');
                let e = stack.entries();
                put_u32(&mut b, e.len() as u32);
                for (c, t) in e {
                    put_u64(&mut b, *c);
                    put_s(&mut b, t);
                }
            }
            4 => {
                let c = stack.pop(arg != 0);
                b.push(b'P');
                put_u64(&mut b, c);
            }
            5 => {
                stack.dump(&mut out);
                b.push(b'D');
            }
            6 => {
                let n = if (1..=1024).contains(&arg) { arg as usize } else { 1024 };
                let (c, code, text) = stack.top(n);
                b.push(b'T');
                put_u64(&mut b, c);
                put_u64(&mut b, code);
                put_s(&mut b, &text);
            }
            _ => {}
        }
    }
    PortRun {
        dump: b,
        out: out.text(Stream::Out).into_bytes(),
        err: out.text(Stream::Err).into_bytes(),
        notices: notices.as_slice().iter().map(|n| n.defect.id).collect(),
        unsupported,
    }
}
