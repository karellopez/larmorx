# The specs of FSL-20/21, AFNI-75 and `cbrtf`, and the AFNI replicas' memory (2026-10-30)

The tools behind a bounded part of 2026-10-30: three behavioural specs written by a spec writer
(`specs/mcflirt.md` §10.6, §12.1, §14.1; `specs/3dUnifize.md` §7, §9.1, §10; `specs/cbrtf.md`), and
two memory fixes (the replica's 3dUnifize, the Python command line). Every script runs from the
checkout with `python -P`; outputs go to the directory given (none is committed but the cbrtf block
hashes).

## The specs' black-box runs

| Script | What it shows |
|---|---|
| `mcflirt_fsl20_21.py <out> [--larmorx \| --replica BIN] [--mode fix\|reproduce]` | mcflirt 6.0.7.23 (or larmorx's original, or the `larmorx-nc` replica) on a 24 × 24 × 16 × 5 phantom: an uncreatable `.mat` directory (FSL-20), unopenable `.par` and `.rms` files (FSL-65, new), histogram costs with fewer than 8 bins (FSL-21), negative `-bins` (FSL-66, new); exit status, messages, files written. `compare_runs.py <fsl-run> <other>...` sets runs side by side |
| `unifize_epi_ball.py <out>` | 3dUnifize `-EPI` on voxels of 3 µm, 1 µm, 0.05, 0.3 and 3 mm: AFNI 26.3.05 under a 2 GiB address-space limit (it dies with signal 11 building the ball), the replica's AFNI-75 guard and the clean-room original, both modes |
| `cbrtf_blackbox.py blocks\|values\|list` | the system C library's `cbrtf` (glibc 2.35 here) through `ctypes`: the hash of each block of 2^20 of the 2^32 inputs (`cbrtf-glibc-2.35-x86_64-blocks.json`, about 2.5 min on 22 workers; hash of the block hashes `0e26908cfda8506a`), single values with the correctly rounded result beside them, a block listed input by input |
| `cbrtf_constants.py` | the binary64 constants of `specs/cbrtf.md` from their decimals |

Findings (for the register, through the handoff): **FSL-65** an output text file that cannot be
opened: `.par` ends the run with "error: unable to open output file!" and status 0 after
`MAT_0000` and the `.rms` files; `.rms` silently not written. **FSL-66** negative `-bins`: mcflirt is
killed by `SIGFPE` (a stage with −1 bins) or aborts with `std::bad_array_new_length` (−2 or fewer),
for every cost function. The `larmorx-nc` replica and the clean-room original run normally on both
today (follow-ups).

## Memory

| Script | What it shows |
|---|---|
| `entry_memory.py` | where `python -m larmorx`'s memory goes on the memory test's small image: the interpreter 9.0 MiB, the Rust core 4.0, the command line's imports (2.3 MiB before 2026-10-30, 0.5 after); 3dAutomask 19.6 MiB through the entry point, 18.5 through `_core.cli_main` alone, 8.4 through the binary, against AFNI's 7.7 |
| `../afni-move/memory.py` | AFNI 26.3.05 against the replicas through the entry point and the binary (`tests/parity/test_memory_peak.py`'s `AFNI_CASES`) |

The replica's 3dUnifize, peak resident memory at one thread (MiB; `afni-move/memory.py`):

| Call | AFNI | before: entry point / binary | after: entry point / binary |
|---|---|---|---|
| `-T2 -clfrac 0.2` on 160 × 192 × 160 | 98.4 to 99.1 | 164.8 / 151.3 | 115.2 / 103.8 |
| `-EPI` on 64 × 64 × 40 × 160 | 109.2 | 320.9 / 307.7 | 120.8 / 110.4 |

What it held that AFNI does not (each now released where AFNI releases it, the arithmetic
unchanged): the input dataset after the float copy of its sub-brick (`DSET_unload`); a full copy of
the image in each contrast inversion (AFNI copies only the mask's values to sort them); the masked
image after its half-size copy (`mri_free(aim)`); a value and a flag per voxel of the local means
(the flags are now recomputed for the few voxels that print a dot); the median after the
white-matter scaling (`free(imin)`); the scale image after `-ssave` (`mri_free(sclim)`); copies of
the `-amsave` and `-ssave` images to write them; and with `-EPI` the whole series in and out side
by side plus a clone of the input (AFNI unloads each input sub-brick once it has its float copy):
float series are now scaled in place, byte and short series split into sub-bricks released one by
one.
