// SPDX-License-Identifier: Apache-2.0
//! The writer's files read back by the reader: groups (compact, dense, compact by a stored
//! phase change), strings, floating-point datasets (contiguous, chunked, empty), and a chunk
//! the deflate filter cannot shrink.

use larmorx_hdf5::write::{Floats, Storage, Writer};
use larmorx_hdf5::{File, Layout, Storage as GroupStorage};

fn floats(f: &File, path: &str, as_double: bool) -> Vec<f64> {
    let info = f.dataset(f.resolve(path).unwrap()).unwrap();
    f.read_floats(&info, as_double, 1, false).unwrap().0
}

fn string(f: &File, path: &str) -> Vec<u8> {
    let info = f.dataset(f.resolve(path).unwrap()).unwrap();
    f.read_strings(&info).unwrap().remove(0).unwrap()
}

#[test]
fn datasets_read_back() {
    let mut w = Writer::new(7);
    let root = w.root();
    w.write_string(root, "empty", b"").unwrap();
    w.write_string(root, "text", b"hello, HDF5").unwrap();
    let g = w.create_group(root, "g").unwrap();
    let a: Vec<f64> = (0..100).map(|i| f64::from(i) / 3.0).collect();
    let b: Vec<f32> = (0..2500).map(|i| i as f32 * 0.5).collect();
    w.write_floats(g, "contiguous", Floats::F64(&a), Storage::Contiguous)
        .unwrap();
    w.write_floats(
        g,
        "chunked",
        Floats::F32(&b),
        Storage::Chunked {
            chunk: 1000,
            level: 5,
        },
    )
    .unwrap();
    w.write_floats(g, "none", Floats::F64(&[]), Storage::Contiguous)
        .unwrap();
    assert!(
        w.write_floats(
            g,
            "zero-chunk",
            Floats::F64(&[]),
            Storage::Chunked { chunk: 0, level: 5 }
        )
        .is_err()
    );
    let bytes = w.finish();
    let f = File::from_bytes(bytes).unwrap();
    assert_eq!(string(&f, "/empty"), b"");
    assert_eq!(string(&f, "/text"), b"hello, HDF5");
    assert_eq!(floats(&f, "/g/contiguous", true), a);
    let back: Vec<f32> = floats(&f, "/g/chunked", false)
        .iter()
        .map(|&x| x as f32)
        .collect();
    assert_eq!(back, b);
    let info = f.dataset(f.resolve("/g/chunked").unwrap()).unwrap();
    assert!(matches!(info.layout, Layout::Chunked { .. }));
    // Three chunks, the last padded with zeros beyond the dataset.
    assert_eq!(f.chunks(&info).unwrap().len(), 3);
    assert!(floats(&f, "/g/none", true).is_empty());
}

#[test]
fn groups_compact_dense_and_large() {
    for n in [8usize, 9, 20, 38] {
        let mut w = Writer::new(0);
        let root = w.root();
        let g = w.create_group(root, "TransformGroup").unwrap();
        for i in 0..n {
            w.create_group(g, &i.to_string()).unwrap();
        }
        let f = File::from_bytes(w.finish()).unwrap();
        let addr = f.resolve("/TransformGroup").unwrap();
        let mut names: Vec<String> = f
            .links(addr)
            .unwrap()
            .into_iter()
            .map(|l| String::from_utf8(l.name).unwrap())
            .collect();
        names.sort_by_key(|s| s.parse::<usize>().unwrap());
        assert_eq!(names, (0..n).map(|i| i.to_string()).collect::<Vec<_>>());
        let dense = matches!(f.group_storage(addr).unwrap(), GroupStorage::Dense);
        assert_eq!(dense, n > 8, "{n} links");
    }
    // More links than the heap's first direct block holds: refused by create_group, kept
    // compact by create_group_for.
    let mut w = Writer::new(0);
    let root = w.root();
    let g = w.create_group(root, "full").unwrap();
    let result = (0..60).try_for_each(|i| w.create_group(g, &i.to_string()).map(|_| ()));
    assert!(result.is_err());
    let names: Vec<String> = (0..60).map(|i| i.to_string()).collect();
    let refs: Vec<&[u8]> = names.iter().map(|n| n.as_bytes()).collect();
    let mut w = Writer::new(0);
    let root = w.root();
    let g = w.create_group_for(root, "big", &refs).unwrap();
    for n in &names {
        w.create_group(g, n).unwrap();
    }
    let f = File::from_bytes(w.finish()).unwrap();
    let addr = f.resolve("/big").unwrap();
    assert_eq!(f.links(addr).unwrap().len(), 60);
    assert!(!matches!(
        f.group_storage(addr).unwrap(),
        GroupStorage::Dense
    ));
}

#[test]
fn a_chunk_the_filter_cannot_shrink_is_stored_as_is() {
    // HDF5's deflate filter fails when the stream would not fit in ceil(1.001 n) + 12 bytes;
    // the optional filter is then skipped for that chunk and its mask bit set.
    let mut w = Writer::with_deflate(0, Box::new(|data: &[u8], _| vec![0u8; data.len() * 2]));
    let root = w.root();
    let v: Vec<f64> = (0..64).map(f64::from).collect();
    w.write_floats(
        root,
        "x",
        Floats::F64(&v),
        Storage::Chunked {
            chunk: 64,
            level: 5,
        },
    )
    .unwrap();
    let f = File::from_bytes(w.finish()).unwrap();
    let info = f.dataset(f.resolve("/x").unwrap()).unwrap();
    let chunks = f.chunks(&info).unwrap();
    assert_eq!((chunks[0].mask, chunks[0].size), (1, 64 * 8));
    assert_eq!(floats(&f, "/x", true), v);
}
