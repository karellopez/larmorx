// SPDX-License-Identifier: Apache-2.0
//! Rewrites an ITK transform file with the writer, replaying ITK's calls with the values and
//! storage read from it and the original's object time, and compares the bytes (a development
//! aid: `cargo run -p larmorx-hdf5 --example itk_rewrite -- in.h5 [out.h5]`).

use larmorx_hdf5::write::{Floats, Storage, Writer};
use larmorx_hdf5::{Datatype, File, Layout, LinkTarget};

fn string(f: &File, path: &str) -> Vec<u8> {
    let a = f.resolve(path).unwrap();
    let info = f.dataset(a).unwrap();
    f.read_strings(&info).unwrap().remove(0).unwrap()
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let input = std::path::Path::new(&args[1]);
    let original = std::fs::read(input).unwrap();
    let f = File::open(input).unwrap();
    let root = f.object_header(f.root()).unwrap();
    let time = root.times.expect("times")[0];
    let mut w = Writer::new(time);
    let r = w.root();
    for name in ["ITKVersion", "HDFVersion", "OSName", "OSVersion"] {
        w.write_string(r, name, &string(&f, &format!("/{name}")))
            .unwrap();
    }
    let tg = w.create_group(r, "TransformGroup").unwrap();
    let tga = f.resolve("/TransformGroup").unwrap();
    let n = f.links(tga).unwrap().len();
    for i in 0..n {
        let g = w.create_group(tg, &i.to_string()).unwrap();
        let base = format!("/TransformGroup/{i}");
        let ttype = string(&f, &format!("{base}/TransformType"));
        w.write_string(g, "TransformType", &ttype).unwrap();
        if String::from_utf8_lossy(&ttype).contains("CompositeTransform") {
            continue;
        }
        for (name, expect_fixed) in [
            ("TransformFixedParameters", true),
            ("TransformParameters", false),
        ] {
            let p = format!("{base}/{name}");
            let a = f.resolve(&p).unwrap();
            let info = f.dataset(a).unwrap();
            let storage = match &info.layout {
                Layout::Chunked { dims, .. } => Storage::Chunked {
                    chunk: dims[0],
                    level: info.pipeline.first().map_or(5, |fl| fl.cd_values[0]),
                },
                _ => Storage::Contiguous,
            };
            let _ = expect_fixed;
            match &info.dtype {
                Datatype::Float(ft) if ft.size == 4 => {
                    let (v, _) = f.read_floats(&info, false, 0, true).unwrap();
                    let v: Vec<f32> = v.iter().map(|&x| x as f32).collect();
                    w.write_floats(g, name, Floats::F32(&v), storage).unwrap();
                }
                _ => {
                    let (v, _) = f.read_floats(&info, true, 0, true).unwrap();
                    w.write_floats(g, name, Floats::F64(&v), storage).unwrap();
                }
            }
        }
    }
    let _ = LinkTarget::Hard(0);
    let ours = w.finish();
    if let Some(out) = args.get(2) {
        std::fs::write(out, &ours).unwrap();
    }
    let first = ours.iter().zip(&original).position(|(a, b)| a != b);
    let ndiff = ours.iter().zip(&original).filter(|(a, b)| a != b).count();
    println!(
        "{}: ours {} bytes, original {} bytes, identical {}, first difference at {:?}, {} bytes differ",
        input.display(),
        ours.len(),
        original.len(),
        ours == original,
        first,
        ndiff
    );
}
