# AFNI 25.2.09 → 26.3.05: the tools behind the version update (2026-10-29)

What changed between the two releases in the code 3dTshift, 3dAutomask and 3dUnifize run, and
what AFNI 26.3.05's new licence means for the replicas. The results are in
[docs/findings/afni-tshift.md](../../../docs/findings/afni-tshift.md) ("AFNI 26.3.05"),
[afni-automask.md](../../../docs/findings/afni-automask.md),
[afni-unifize.md](../../../docs/findings/afni-unifize.md) and
[afni-relicensing-26.md](../../../docs/findings/afni-relicensing-26.md). These tools read AFNI's
source and binaries: replica authors and spec writers only (CLAUDE.md rule 1).

Run them from the repository with `LARMORX_WORKSPACE` set (or from a checkout inside the
workspace); the release trees are `<workspace>/tags/AFNI-25.2.09` and `AFNI-26.3.05`, the oracle
builds `<workspace>/oracles/afni-25.2.09` and `afni-26.3.05` (`scripts/build_afni_oracle.sh`).

| Tool | What it shows | Result (2026-10-29) |
|---|---|---|
| `diff_tags.py [--out DIR] [--all]` | every `src/` file the AFNI `PROVENANCE.md` files name, diffed between the two trees (changed lines, of which licence text); `--all` every changed `.c`/`.h` of `src/` | 56 named files: 23 unchanged; 27 licence header only; code in `3dTshift.c` (AFNI-05), `thd_niftiread.c` (AFNI-03), `thd_automask.c` (a new `THD_mask_erode2D`), `3ddata.h` and `mrilib.h` (declarations, includes), `AFNI_atlas_spaces.niml` (new atlases; no template space changed) |
| `function_diff.py <old bin> <new bin> [--strings] [--show F]` | every function of the three programs' binaries (both GCC 11.4 `-O2` builds) compared by machine code, addresses masked: the routines and inlined macros whose code changed, wherever their file | 3dTshift: 3,280 identical, 8 changed; 3dAutomask 3,382 and 7; 3dUnifize 3,412 and 7. Changed: `THD_open_nifti` (AFNI-03), 3dTshift's `main` (AFNI-05), `machdep` (a macOS check, `isMacTahoe`, which returns 0 on Linux), and five help-search functions (`approx_str_sort_all_popts`, `form_C_progopt_string_from_struct`, `get_updated_help_file`, `progopt_C_array`, `unescape_unix_str`) that only AFNI's `-h_*`/`-all_opts` paths reach; new: `THD_mask_erode2D` (called by `mri_warp3D_align.c` only), `GetAfniSpreadsheetViewer`, `isMacTahoe` |
| `help_diff.sh <work> [old] [new]` | `-help` of the three programs in both builds | identical but the line naming the build and its date |
| `licence_inventory.py [--markdown]` | the licence statement of every AFNI file the replicas derive from, in both releases, word for word | 30 MCW files GPL-2 → CC BY 4.0; 24 without a notice (NIH) and the NIfTI library's public domain unchanged; none GPL in 26.3.05 |
| `glibc_permissive/` | glibc 2.35's `sinf` and `cosf` against Arm's Optimized Routines v19.11 (MIT), compiled as glibc compiles its x86-64 builds, on all 2^32 inputs (`run.sh <work>`, which fetches Arm's files with `fetch_aor.sh`) | not run here (compiling downloaded source needs the user's permission); the source comparison and the FMA build's exhaustive check (`crates-nc`) are in the relicensing finding |

The parity suites' **version step** (`larmorx_validation.parity.afni_versions`) runs AFNI 25.2.09
beside 26.3.05 on every case of the replica records and names each case whose AFNI output changed,
with its reason; the records carry it.
