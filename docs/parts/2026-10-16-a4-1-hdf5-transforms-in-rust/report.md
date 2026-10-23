# Final report: A4.1: HDF5 transforms in Rust

Agent `a5b5185d32a7fb70f`, 2026-10-16. The agent's report to the orchestrator, verbatim.

## A4 part 1 is done: ITK's `.h5` transform files are read and written in pure Rust, with no HDF5 C library and no h5py

All gates pass on the branch's last commit `926913e` (branch `worktree-agent-a5b5185d32a7fb70f`). Nothing is pushed or merged, and no commit has an AI attribution.

### What exists now
- **Rust crate `crates/larmorx-hdf5`** (`#![forbid(unsafe_code)]`).
  - **Reader.** Covers everything HDF5 1.12 can store inside one file:
    - superblocks 0 to 3; object headers v1 and v2 with continuation chunks;
    - symbol-table, compact and dense groups. Dense groups cover every fractal-heap layout (direct, indirect, huge and tiny objects, with or without filters) and links indexed by name or by creation order.
    - every chunk index: v1 B-tree, single chunk, implicit, fixed array, extensible array, v2 B-tree;
    - the deflate, shuffle, Fletcher-32, n-bit and scale-offset filters;
    - floats of any size and byte order, converted as HDF5 1.12.1 converts them;
    - fixed and variable-length strings (ASCII and UTF-8), fill values;
    - committed datatypes, and messages shared in the file's shared-message heaps (SOHM).
  - **Writer.** It replays HDF5 1.12.1's file layout for ITK's sequence of calls: file-space aggregators and free sections, object-header allocation and condensing, the global heap, the chunk B-tree, and conversion of a group to dense storage at the 9th link.
- **`larmorx_io::itk_hdf5`**: reads and writes as ITK 5.4.5 does, with ANTs' error messages.
- **Command line:** `larmorx ants antsApplyTransforms -t x.h5` and `-t [x.h5,1]` now work.
- **Python:**
  - `lx.transforms.read` / `write(path, transforms, hdf5=Hdf5WriteOptions(...), upstream_defects=...)`;
  - `lx.transforms.load_itk_composite` reads as nitransforms does;
  - h5py is no longer a dependency;
  - `.pyi` stubs are updated.
- **Object times:** larmorx writes a fixed 0 by default (`Hdf5WriteOptions.time`), never the current time. This is documented in the API page and the validation record.

### Not covered
- **Data stored in other files** (external raw files, external links, virtual datasets): larmorx refuses them with "unsupported HDF5 feature: …", in ITK's message form on the command line. ITK's HDF5 follows them. These are recorded as expected divergences.
- **Writer, 38 or more transforms in one composite:** HDF5 gives the group's fractal heap an indirect block, which larmorx does not replay. larmorx stores the links compactly instead, and every reader reads the same transforms. This is an expected divergence.
- **antsApplyTransforms transform outputs** (`-o Linear[...]`, `-o CompositeTransform[...]`, `-o DisplacementField[...]`) are still refused. The writer is ready for them; this is listed in `docs/next-steps.md`.
- I found and fixed a reader bug: the layout message's chunk-index type numbers were shifted. No file ITK writes uses those indices, so no transform file was affected.

### Parity
**`docs/validation/itk-hdf5.md`**, standard tier, recorded from clean commit `729c5c2`: 144 cases, 0 failures (123 pass, 16 rejected by both, 5 expected divergences).
- Reading:
  - every dataset matches h5py bit for bit in 73 of 73 files;
  - the HDF5 storage-feature files match h5py in 49 of 49;
  - transforms as ITK reads them match bit for bit in 57 of 57 files, in double and in float;
  - nitransforms' chain matches in 57 of 57.
- 16 of 16 files ITK refuses are refused alike (exit status and messages).
- 4 files ITK reads are refused by larmorx: one is ITK-17 (ITK casts a 2-D component into a 3-D composite, which is undefined behaviour); the other three are the data-in-other-files cases.
- The 5th expected divergence is the writer's 38-affine composite.
- **Writing, byte comparison with ITK:**
  - 16 of 17 files ITK writes are reproduced byte for byte by larmorx when given ITK's own compressed chunks and object times; the 17th is the 38-affine case.
  - With larmorx's own deflate (zlib-rs), 12 of 17 are byte-identical.
  - In the other 5, the only differences are the large fields' deflate streams (decompressed, they are identical), plus:
    - each stream's size in the chunk B-tree;
    - every address allocated after a stream, with the checksums of the object headers holding them;
    - the superblock's end-of-file address and checksum.
  - Separately from deflate, larmorx's object times (0) differ from ITK's, and so do the header checksums that cover them.
- ANTs (same output image), h5py and nitransforms read larmorx's files as they read ITK's.
- The record and the API page list these byte differences exactly.

**`docs/validation/ants-apply-transforms.md`**, recorded from `be0c0eb`: 182 cases, including 16 new `.h5` cases (8 scenarios × double and `--float 1`).
- 174 compared, 8 rejected by both.
- 160 of 174 are bit-identical to the unmodified ANTs, including all 89 `--float 1` cases; 174 of 174 with CORE-MATH preloaded.
- The 14 that differ do so only through glibc's maths. Two of them go through fMRIPrep's `.h5` composite.

### Golden and the rest of the contract
- **Golden case `py.transforms.write_h5`** (golden suite: 103 cases, recorded at `01dbc56`):
  - three files: a compressed composite with a 32³ field, the same uncompressed, and a dense group of 11 affines;
  - each is checked by its whole-file bytes and by a fingerprint of the decoded content;
  - `docs/validation/golden.md` says which.
  - **Gap:** the reader paths added since (extensible array, n-bit, scale-offset, filtered heaps, SOHM) have no golden case. Making their input files needs the HDF5 C library, which CI does not have, so they are checked only by Linux parity. Committing small fixture `.h5` files would close this, but I did not commit binaries without your decision.
- **Docs:**
  - API page `docs/api/transforms-hdf5.md`, with the option mapping;
  - findings `docs/findings/itk-hdf5.md`;
  - provenance in `crates/larmorx-hdf5/PROVENANCE.md` and `crates/larmorx-io/PROVENANCE.md`;
  - `docs/next-steps.md` §1, the CLAUDE.md queue item 6, and the validation index row.
- **Register** (`docs/findings/upstream-defects.md`): ITK-12 (c), ITK-13 (f), ITK-14 (c), ITK-15 (c), ITK-16 (a), ITK-17 (a), HDF-01 (c), PIPE-17 (c). The class (c) and (f) entries have D13 switches; ITK-16 and ITK-17 are refused.

### Gates on `926913e`
- Root Cargo workspace: fmt, clippy `-D warnings`, `cargo test --locked` (409 passed).
- `crates-gpl`: fmt, clippy, test (68 passed); its lockfile did not need to change.
- ruff check and format, `check_spdx.py`.
- `pytest tests/python tests/golden`: 356 passed.
- `pytest tests/parity` smoke tier: 1940 passed.
- CI has not run, because nothing was pushed.

### Benchmark: still pending
The machine was not quiet, so there is no benchmark record. Informal timings on one run, not a record:
- fMRIPrep's 90 MB T1w→MNI composite: larmorx reads it in 0.38 s, h5py in 0.46 s, nitransforms in 1.16 s; larmorx writes it in 2.97 s.
- A dense 37-affine composite: larmorx reads it in 36 ms, h5py in 22 ms. Each path lookup in a dense group walks all its links; it could be cached later.

### Commits
`267d875`, `8f4a001`, `f0be089`, `ee0919d` (WIP checkpoints), `60c569e`, `01dbc56`, `145e910`, `dda8777`, `7625d6b`, `132eac0`, `32ee029`, `729c5c2`, `be0c0eb` (itk-hdf5 record), `1d6fb64` (ants-apply-transforms record), `926913e`.

Scratch notes and oracle files are in `/tmp/claude-1000/-home-karelo-VScode-projects-super-fmriprep/d96e838f-0213-4978-9a72-dac8eae4b541/scratchpad/a4-hdf5/NOTES.md`. Run logs are in `/home/karelo/VScode_projects/super_fmriprep/oracles/logs/a4-hdf5-*.log`.
