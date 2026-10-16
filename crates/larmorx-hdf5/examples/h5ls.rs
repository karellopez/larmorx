// SPDX-License-Identifier: Apache-2.0
//! Lists an HDF5 file's groups and datasets with what the reader finds in them (a development
//! aid: `cargo run -p larmorx-hdf5 --example h5ls -- file.h5`). Float datasets are printed as
//! the SHA-256-free FNV-1a hash of their f64 bits, so outputs can be compared with h5py's.

use larmorx_hdf5::{Datatype, File, LinkTarget, ObjectKind};

fn fnv(values: &[f64]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for v in values {
        for b in v.to_bits().to_le_bytes() {
            h ^= u64::from(b);
            h = h.wrapping_mul(0x0100_0000_01b3);
        }
    }
    h
}

fn walk(f: &File, addr: u64, path: &str) {
    for link in f.links(addr).expect("links") {
        let name = String::from_utf8_lossy(&link.name).into_owned();
        let p = format!("{path}/{name}");
        let target = match &link.target {
            LinkTarget::Hard(a) => *a,
            other => {
                println!("{p}: link {other:?}");
                continue;
            }
        };
        match f.kind(target).expect("kind") {
            ObjectKind::Group => {
                println!("{p}/ (group, {:?})", f.group_storage(target).unwrap());
                walk(f, target, &p);
            }
            ObjectKind::Dataset => {
                let info = f.dataset(target).expect("dataset");
                match &info.dtype {
                    Datatype::Float(ft) => {
                        let (v, _) = f.read_floats(&info, ft.size == 8, 0, true).expect("read");
                        println!(
                            "{p}: float{} {:?} {:?} n={} fnv={:016x} first={:?}",
                            ft.size * 8,
                            info.space.dims(),
                            info.layout_kind(),
                            v.len(),
                            fnv(&v),
                            &v[..v.len().min(3)]
                        );
                    }
                    Datatype::VarLen { .. } | Datatype::String { .. } => {
                        let s = f.read_strings(&info).expect("strings");
                        let s: Vec<String> = s
                            .iter()
                            .map(|x| {
                                x.as_ref().map_or("<null>".into(), |b| {
                                    String::from_utf8_lossy(b).into_owned()
                                })
                            })
                            .collect();
                        println!("{p}: string {:?} {s:?}", info.space.dims());
                    }
                    other => println!("{p}: {other:?}"),
                }
            }
            k => println!("{p}: {k:?}"),
        }
    }
}

fn main() {
    for arg in std::env::args().skip(1) {
        let f = File::open(std::path::Path::new(&arg)).expect("open");
        println!("== {arg} (superblock {})", f.superblock().version);
        walk(&f, f.root(), "");
    }
}
