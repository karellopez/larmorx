# Final report: The FSL BET replica, its spec, and its merge into larmorx-nc-core

Agent `a38ebb82788afa285`, 2026-10-24 to 2026-10-26. Its three reports to the orchestrator: the
part itself (summary), the merge step after the mcflirt replica (verbatim in essence), and the
FSL-22 follow-up. The detailed texts for every shared document are in `handoff.md`.

## Part 1 (2026-10-24): the replica and the spec

- **Coverage:** the replica in `crates-nc/larmorx-nc-bet`, from FSL 6.0.7.17's source: bet2 (every
  option, message, exit status, output format); bet (`-R`, `-F`, `-Z`, `-d`, its argument handling,
  error trap and cleanup, the parts of avwutils it runs, fslpy's imtest/remove_ext/imglob, a `dc`);
  betsurf (T1+T2 and `--t1only`). glibc 2.35's `pow`/`exp` and fdlibm's `tanh` bit for bit.
  Pending: `bet -S`, `-B`, `-A`, `-A2` (they need standard_space_roi, FLIRT and FAST; they refuse
  clearly).
- **Parity:** standard 129 cases (93 bit-identical, 32 both reject or FSL crashes, 4 pending), full
  143 (107 pass, 320 of 320 files byte-identical); fMRIPrep's 9 BET calls on its five oracle runs,
  18 of 18 files.
- **Register:** FSL-40..57, by rules A-D, none undecided. `specs/bet.md` (§1-§11, behaviour only).
- **Site:** a new BET page (4 examples, every metric, verdicts), no face shown.

## Part 2 (2026-10-26): the merge into the shared core

- One translation of each FSL library in `larmorx-nc-core` (BET's and mcflirt's newimage, NewNifti,
  Armadillo, fdlibm, cnum and x87 joined; `volume<T>` generic); BET's meshclass, options, avwutils,
  istream, stdsort, `pow`/`exp`, libm-verify moved in; `larmorx-nc fsl bet|bet2|betsurf` replaces the
  stand-in program; one `fsl-source/` with one MANIFEST.
- **NaN bits:** BET's arithmetic follows SSE's NaN rules (the NaN matrix: 60 cases, 0 differ).
- **D13 failure cases:** FSL-41, FSL-52, FSL-43's crash class and the new FSL-58 refused by default;
  `reproduce` fills what FSL leaves undefined and names the defect; FSL-22's warning added.
- **Licence:** `pow` and `exp` translated from Arm's Optimized Routines (MIT) instead of glibc's
  LGPL files (tables and code identical but for macro names): no LGPL text left in crates-nc.
- **Correction:** `.nii.gz` files are FSL's after decompression, not byte for byte compressed.
- **Oracle cache:** `inputs=` added; BET's derived inputs in `oracles/inputs/fsl-bet/`.
- **Memory** (never above FSL): bet2 84 MiB vs 118, bet -R 106 vs 121, betsurf 190 vs 240, bet -F
  on 240 volumes 226 vs 264 (saving now streams a volume's values; mcflirt shares this).
- **Generated tables:** BET in `validation/tools.toml`. **Random testing:** a descriptor; four
  trials found 8 kinds of difference, all fixed, 7 kept as permanent cases (FSL-58 new).
- **Records** (clean commit 8e0059a, 0 failures): standard 159 cases (115 bit-identical, 278 of 278
  files, 30 rejected by both, 10 where FSL crashes and `reproduce` fills the undefined, 4 pending);
  full 173 (129 bit-identical, 364 of 364 files); fMRIPrep's 29 BET calls on all 12 oracle runs
  with BOLD runs agree, also with fMRIPrep's own outputs (58 of 58).
- **mcflirt unchanged:** standard 196 cases, 175 pass, 21 both reject, 0 status or detail
  differences from its record.
- **Gates:** `gates-rust.sh all`; Python 599; parity smoke 2461; golden: crates-nc re-recorded
  (BET's 10 cases, 63 outputs; mcflirt's 179 unchanged), main 153 cases / 892 outputs, every
  existing value unchanged.
- **Site:** BET page (4 examples, 173 cases, 27 images), every other page unchanged; number check
  44 of 44; blob branch 322 blobs.

## Follow-up (2026-10-26): FSL-22 reproduced exactly

At the orchestrator's request, `reproduce` uses the core's `std::sort` emulation for `calc_bval`
(written from the SGI STL, its HP/SGI notice in NOTICE; the one step where libstdc++'s order
differs written from that step's description, no libstdc++ text). C's NaN matrix: 160 of 160 cases
give mcflirt's bytes in `reproduce` (before: 32 differed); the last one, `bgnan-stats`, also needed
mcflirt's `-stats` sums in the binary's operand order (confirmed by disassembly), now fixed. A
probe (`validation/probes/libstdcxx-sort/`) checks the emulation against GCC 11.4 and 14.3's own
`std::sort` on 768 seeded vectors (495 with NaNs). The default still puts NaNs last.
