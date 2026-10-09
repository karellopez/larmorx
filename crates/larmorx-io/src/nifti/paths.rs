//! File names: single `.nii` / `.nii.gz` files and `.hdr` + `.img` pairs.

use std::path::{Path, PathBuf};

/// Where the header and the voxel data of an image live.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NiftiPaths {
    /// Header and data in one file (`.nii`, `.nii.gz`).
    Single(PathBuf),
    /// A header file and a data file (`.hdr` + `.img`, optionally both `.gz`).
    Pair { header: PathBuf, image: PathBuf },
}

impl NiftiPaths {
    /// Classifies `path` by its extension (case-insensitive). For a pair, either file may be
    /// given; the other one keeps the case and the compression suffix of the one given.
    pub fn from_path(path: &Path) -> Option<NiftiPaths> {
        let name = path.file_name()?.to_str()?;
        let lower = name.to_ascii_lowercase();
        let stem_len = if lower.ends_with(".gz") {
            name.len() - 3
        } else {
            name.len()
        };
        let base = &name[..stem_len];
        let base_lower = &lower[..stem_len];
        let ext_start = base
            .len()
            .checked_sub(4)
            .filter(|&i| base.is_char_boundary(i))?;
        let (stem, ext) = base.split_at(ext_start);
        let gz_suffix = &name[stem_len..];
        let with_ext = |e: &str| path.with_file_name(format!("{stem}{e}{gz_suffix}"));
        match &base_lower[ext_start..] {
            ".nii" => Some(NiftiPaths::Single(path.to_path_buf())),
            ".hdr" | ".img" => {
                let upper = ext.chars().skip(1).all(|c| c.is_ascii_uppercase());
                let (h, i) = if upper {
                    (".HDR", ".IMG")
                } else {
                    (".hdr", ".img")
                };
                Some(NiftiPaths::Pair {
                    header: with_ext(h),
                    image: with_ext(i),
                })
            }
            _ => None,
        }
    }

    /// The file that holds the header.
    pub fn header_path(&self) -> &Path {
        match self {
            NiftiPaths::Single(p) => p,
            NiftiPaths::Pair { header, .. } => header,
        }
    }

    /// The file that holds the voxel data.
    pub fn image_path(&self) -> &Path {
        match self {
            NiftiPaths::Single(p) => p,
            NiftiPaths::Pair { image, .. } => image,
        }
    }

    pub fn is_single(&self) -> bool {
        matches!(self, NiftiPaths::Single(_))
    }
}

/// Whether a name asks for gzip compression (ends with `.gz`, any case).
pub fn wants_gzip(path: &Path) -> bool {
    path.file_name()
        .and_then(|n| n.to_str())
        .is_some_and(|n| n.to_ascii_lowercase().ends_with(".gz"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_names() {
        assert_eq!(
            NiftiPaths::from_path(Path::new("a/b.nii.gz")),
            Some(NiftiPaths::Single("a/b.nii.gz".into()))
        );
        assert_eq!(
            NiftiPaths::from_path(Path::new("b.NII")),
            Some(NiftiPaths::Single("b.NII".into()))
        );
        assert_eq!(
            NiftiPaths::from_path(Path::new("x/y.img.gz")),
            Some(NiftiPaths::Pair {
                header: "x/y.hdr.gz".into(),
                image: "x/y.img.gz".into()
            })
        );
        assert_eq!(
            NiftiPaths::from_path(Path::new("Y.HDR")),
            Some(NiftiPaths::Pair {
                header: "Y.HDR".into(),
                image: "Y.IMG".into()
            })
        );
        assert_eq!(NiftiPaths::from_path(Path::new("y.mgz")), None);
        assert_eq!(NiftiPaths::from_path(Path::new("nii")), None);
        assert!(wants_gzip(Path::new("a.nii.GZ")) && !wants_gzip(Path::new("a.nii")));
    }
}
