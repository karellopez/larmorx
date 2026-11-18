# Provenance: larmorx-freesurfer-volumes

**Replica: translated from FreeSurfer's source.** This crate holds wave 3.2's programs of the FreeSurfer
replica (`docs/freesurfer/waves.md` section 4): the volume programs both streams run (`mri_convert`, `mri_info`, `mri_normalize`, `mri_segment`, `mri_fill`, ...), fMRIPrep's `mri_coreg`, `mri_robust_register` and `mri_robust_template`, and the scripts around them (`mri_nu_correct.mni`, `seg2cc`, ...). It is distributed under FreeSurfer's
licence (`LicenseRef-FreeSurfer`, `../LICENSE`: the FreeSurfer Software License with its required
preface), as part of the `larmorx-freesurfer` package (`docs/licensing.md`).

The upstream is FreeSurfer 8.2.0 (GitHub's `v8.2.0` archive, SHA-256
`bb1699f7655ebf0f3790db6a78af9532ea7c832c97fe5f71bfd9f0e9526d7e78`; `docs/freesurfer-plan.md`),
built for the official binaries with GCC 8.4.0 (`docs/freesurfer/variation.md`).

## FreeSurfer files and functions translated

Created empty by queue item 3.1-01 (`docs/freesurfer/queue.md`); each item that brings code adds
its files and functions here, with the third-party code it found in them (CLAUDE.md rule 3: each
ported file is audited for third-party code first).

| FreeSurfer 8.2.0 | What | Here |
|---|---|---|
| `include/csv.h` (queue item 3.2-04, track A relay 46) | `CSVState`, `readCSVRow`, `readCSV` | `src/mri_convert/csv.rs` |
| `utils/stats.cpp` 53-415 (3.2-04, taken from 3.2-07) | `LoadStatTableFS`, `LoadStatTableCSV`, `LoadStatTableTSV`, `LoadStatTable`, `AllocStatTable`, `WriteStatTable`, `PrintStatTable`, `InitStatTableFromMRI` (FS-949, FS-950, FS-1031 to FS-1037), `STAT_TABLE` (`include/stats.h`) | `src/mri_convert/stat_table.rs` |
| `utils/fsgdf.cpp` 868-886 (3.2-04) | `gdfCountItemsInString` (FS-948) | `src/mri_convert/stat_table.rs` |
| `mri_convert/mri_convert.cpp` 57-73, 79-1882, 3889-4001 (3.2-04 and 3.2-05, first half) | the file statics, `main` from its start to the check of a GDF image stem (the option loop, its checks, the parse-only report; FS-1038 to FS-1042), `get_ints`, `get_floats`, `get_string`, `usage_message`, `usage`; `include/DICOMRead.h`'s globals the options set (`DicomSettings`) | `src/mri_convert/options.rs` |
| `mri_convert/mri_convert.help.xml` | the help text `usage` renders (FreeSurfer's file, copied unchanged) | `src/mri_convert/mri_convert.help.xml` |

Third-party code: none in these files (FreeSurfer's own, MGH's notice; `csv.h` carries no notice
and is FreeSurfer's). Their library calls are the other crates' (`larmorx-freesurfer-io`'s TSV
reader, file names, format identification and `MRIwrite`; `-core`'s matrices, version and help;
`-volume`'s orientation check) and `larmorx-core`'s `strtod`/`strtof` (C's conversions, black box).

## Changes from the originals

- `main`'s variables are one struct (`Options`), the unset ones `None` until an option sets them;
  the DICOM reader's globals the options set are kept in it (`DicomSettings`) until the DICOM
  reader is ported (queue item 3.7-14); `setenv`/`unsetenv` act on the program's environment
  table (`run::Environment`).
- `std::stof`'s `ERANGE` comes from fs.core's scanner on the characters `strtof` read (glibc's
  `strtof` and `%f` share the conversion).
- FS-948 (the item count's read past the line: the count kept, no notice), FS-949 and FS-950 (rule
  A), FS-1031 to FS-1042 (crashes and memory: both modes as `specs/mri_convert.md` says).
