// SPDX-License-Identifier: LicenseRef-FSL
// Copyright 2026 Karel Lopez Vilaret (the translation).
//! newimage's file names (fsl-newimage 2203.12, `generalio.cc`): the image extensions, the base
//! name of a file, and how a name given to a program finds its file.

/// newimage's image extensions: (header-bearing file, data-bearing file), in search order.
pub const EXTENSIONS: [(&str, &str); 4] = [
    (".nii.gz", ".nii.gz"),
    (".nii", ".nii"),
    (".hdr", ".img"),
    (".hdr.gz", ".img.gz"),
];

/// `make_basename`: the name without one image extension (the first that matches, in the
/// order above).
pub fn make_basename(name: &str) -> String {
    for (h, d) in EXTENSIONS {
        if let Some(b) = name.strip_suffix(h) {
            return b.to_owned();
        }
        if let Some(b) = name.strip_suffix(d) {
            return b.to_owned();
        }
    }
    name.to_owned()
}

/// `std::filesystem::is_regular_file` (following symbolic links).
pub fn is_regular_file(path: &str) -> bool {
    !path.is_empty() && std::fs::metadata(path).is_ok_and(|m| m.is_file())
}

/// `return_validimagefilename(filename)`: the file a name refers to, or the message of the
/// exception newimage throws; each failure first prints its `Image Exception` line to
/// `printed`.
pub fn return_validimagefilename(name: &str, printed: &mut String) -> Result<String, String> {
    let bname = make_basename(name);
    if bname != name {
        let mut valid = String::new();
        for (h, d) in EXTENSIONS {
            if format!("{bname}{h}") == name || format!("{bname}{d}") == name {
                valid = format!("{bname}{h}");
            }
        }
        if is_regular_file(&valid) {
            return Ok(valid);
        }
    }
    let mut valid = String::new();
    let mut count = 0;
    for (h, _) in EXTENSIONS {
        let candidate = format!("{bname}{h}");
        if is_regular_file(&candidate) {
            valid = candidate;
            count += 1;
        }
    }
    if count > 1 {
        let msg = format!("Multiple possible filenames detected for basename: {bname}");
        printed.push_str(&format!("Image Exception : #61 :: {msg}\n"));
        return Err(msg);
    }
    if count == 0 {
        let msg = format!("No image files match: {bname}");
        printed.push_str(&format!("Image Exception : #63 :: {msg}\n"));
        return Err(msg);
    }
    Ok(valid)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base_names() {
        assert_eq!(make_basename("a/b.nii.gz"), "a/b");
        assert_eq!(make_basename("b.img"), "b");
        assert_eq!(make_basename("b.hdr.gz"), "b");
        assert_eq!(make_basename("b.img.gz"), "b");
        assert_eq!(make_basename("b.nii.gz.nii"), "b.nii.gz");
        assert_eq!(make_basename("b"), "b");
    }

    #[test]
    fn finding_files() {
        let d = tempfile::tempdir().unwrap();
        let base = d.path().join("x").to_string_lossy().into_owned();
        let mut printed = String::new();
        assert!(return_validimagefilename(&base, &mut printed).is_err());
        assert!(printed.contains("#63 :: No image files match"));
        std::fs::write(format!("{base}.nii.gz"), b"").unwrap();
        assert_eq!(
            return_validimagefilename(&base, &mut String::new()).unwrap(),
            format!("{base}.nii.gz")
        );
        std::fs::write(format!("{base}.nii"), b"").unwrap();
        let mut printed = String::new();
        assert!(return_validimagefilename(&base, &mut printed).is_err());
        assert!(printed.contains("#61 :: Multiple possible filenames"));
        // a full name that exists is taken as it is
        assert_eq!(
            return_validimagefilename(&format!("{base}.nii"), &mut String::new()).unwrap(),
            format!("{base}.nii")
        );
    }
}
