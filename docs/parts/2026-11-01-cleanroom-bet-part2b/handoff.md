# HANDOFF: BET's clean-room original, part 2b (2026-10-31 and 11-01)

Branch: the worktree `agent-a888f8be0d1f2c1af` (from main `eac8c09`). Site branch: `cleanroom-bet`
in the clone `<scratch>/cleanroom-bet-2b/site`. Exact texts for the shared documents follow; my own
files (crate, PROVENANCE, API page, findings, record, benchmark, investigations, evidence module,
golden registry and checksums, `validation/tools.toml`'s fsl-bet lines, the site's fsl-bet page and
evidence data) are in the branches.

GLIBC-06 (the user's decision of 2026-10-31, option c): `--tanh correctly-rounded` (default
`glibc`) on `larmorx fsl bet2` and `bet`, `lx.fsl.bet(tanh=...)`; **the GLIBC-06 notice machinery
was removed** (the second surface computed only to warn; with the option the user asks for the
correctly rounded `tanh`, so no notice is needed). One golden case, `cli.fsl.bet2.tanh`.

## 0. Gates and checks (at e63df59 / f847484)

Rust gates of the three workspaces (fmt, clippy -D warnings, tests): pass (`oracles/logs/gates-rust-cleanroom-bet-2b-2026-11-01-b.log`); the parity smoke tier: 2,610 passed (`oracles/logs/parity-smoke-cleanroom-bet-2b-2026-11-01.log`); pytest tests/python tests/golden validation/tests, ruff, ruff format, SPDX: pass; the record re-made from e63df59 (`compare_records`: every case as at bdbf2db; against main's record: the 142 cases unchanged, 26 new); golden: 4 new cases, none changed; the site page's inventory after >= before (`inventory-before.json`, `inventory-after.json` in this folder), `LX_EVIDENCE.check()` 247 of 247 numbers (76 before), `checkViewers()` 100 of 100 (70 before); screenshots in `shots_before/`, `shots_after/`, `shots_sel/`, `shots_ex/`.

## 1. `docs/findings/upstream-defects.md` (the register): behaviour only

No new entry. Two additions, behaviour only:

- In the summary table's FSL-50 row, after "the profile dropped (rule A, 2026-10-24)", add:
  "; random testing 2026-10-31 (cases 216 and 635): a profile FSL sets aside without stopping
  where the clean-room original's `reproduce` stops; when FSL stops and when it only sets the
  profile aside is a question for the spec writer".
- Wherever the register's text on BET says BET has no clean-room original or that betsurf's is to
  come: "BET's clean-room original (`crates/larmorx-fsl`: `bet2` and the `bet` script since
  2026-10-30, `betsurf` since 2026-10-31) shares the replica's corrections and `reproduce` switches
  of FSL-40 to FSL-58 and FSL-60 to FSL-63 (FSL-49 and FSL-57 kept as FSL's)."

Two behaviours found by the trial and checked with FSL as a black box are not defects and need
no entry (they are FSL's normal behaviour, now followed by the original; for the spec writer, §7):
`bet2 -c 0 0 0` is "no centre given" (the centre of gravity), and `bet -F -d` keeps a float64
series's first volume `<out>_tmp` as float64.

## 2. `docs/api/upstream-defects.md`

Replace the sentence "BET has no clean-room original yet." (in the paragraph after the table,
"Rows FSL-40 to FSL-58 are BET's replica's ...") with:

> BET's clean-room original (`crates/larmorx-fsl`; `larmorx fsl bet`, `bet2`, `betsurf`, and
> `lx.fsl.bet`, `lx.fsl.betsurf` with `implementation="original"`, or `"auto"` without
> `larmorx-nc`) shares these corrections and their `reproduce` switches: `bet2` and `bet` since
> 2026-10-30, `betsurf` (FSL-43, -47, -48, -50 to -55, -58) since 2026-10-31.

## 3. `docs/next-steps.md`, item 7

Replace from "**Part 2b (next):** betsurf's original (spec §6), the site page's clean-room
panels, a fresh benchmark, and BET's default `tanh` ..." (its current wording, which since the
decision's merge names the decided option) up to, not including, "**For the spec writer (one
question):**" with:

> **Part 2b: done (2026-10-31 and 11-01).** betsurf's clean-room original complete (spec §6:
> every option, message and failure case; FSL-43, -47, -48, -50 to -55, -58 with `reproduce`;
> `larmorx fsl betsurf`, `lx.fsl.betsurf(implementation="original")`, and `"auto"` without
> `larmorx-nc`); its profile rules settled by a black-box oracle of single profiles (330 of 330
> decided as FSL decides); GLIBC-06 as the user decided (`--tanh correctly-rounded` on request,
> the notice removed); record [fsl-bet](validation/fsl-bet.md) re-made with betsurf's real heads:
> 127 of 128 compared cases within the thresholds (ds002603's scalp with its T2 misses: boundary p95 2 voxels), all 217 bet2 and bet brains, masks and outlines bit-identical; the site page's clean-room panels; benchmark [fsl-bet](benchmarks/fsl-bet.md)
> re-made (the original 2.5 to 3.6 times FSL's speed on one thread, 6.5 to 13.5 on 24); random
> testing's trial: two corrections of the original (`-c 0 0 0`, `bet -F`'s float64 `_tmp`).
> **Open, for the spec writer (BET's questions,
> [findings/fsl-bet-original.md](findings/fsl-bet-original.md)):** (1) the centre's conversion
> on a neurologically stored image (cases 622 and 677, and now 17 of the trial's `bet`/`bet2`
> cases: `bet -R`, `-c`); (2) betsurf's deformation where few profiles vote (the inner skull up to
> 11 mm on a few vertices of a skull base) and the scalp's small differences everywhere; (3) what
> betsurf's profiles read on small images (the trial's 126 differing cases); (4) when FSL-50
> stops the program and when FSL only sets the profile aside (cases 216, 635).

and change "**For the spec writer (one question):**" to "**For the spec writer (the first of
the four above, in detail):**".

## 4. Random testing: triage notes by case (`validation/difftest/triage.toml`)

Trial `oracles/difftest/2026-10-31-cleanroom-betsurf` (seed 20261031, 1,846 cases, from
`21762e8`, 16 cases at a time, one thread each; `report.md` there). Each a clean-room original's
difference, with its reproducer: the finding `findings/fsl-bet/20261031-<n>-original.json`, and
`validation/investigations/cleanroom-bet/night_case.py 20261031 <n> <dir>`.

- **Replica:** agrees in every case it finished (1,843); timeouts 52, 575, 1028 (the per-case
  limit; 52 and 1028 time out in the original too).
- **fixed (2026-11-01, e63df59; to promote as permanent cases):** 1608 (`bet -R` on an image of
  negative values: the script passes `-c 0 0 0`, FSL's "no centre"), 1210 (the same path: FSL's
  second bet2 then crashes and the script reports it), 1740 (`bet -R`, the same path), 1053
  (`bet -F -d` on a float64 series: `<out>_tmp` float64). `retrial.py` with the fix: these four
  agree (`oracles/logs/retrial-cleanroom-bet-2b-2026-11-01.log`). I did not promote them myself:
  the permanent cases' folder is the replica authors', which my role does not open.
- **open, spec question 1 (a centre on a neurologically stored image):** bet 30, 37, 196, 280,
  350, 366, 383, 735, 899, 965, 1011, 1044, 1457, 1708, 1822; bet2 1108, 1676. 1457 stored the
  other way along x gives FSL's result exactly; 965's second pass starts from FSL's printed centre
  `-0.000001 0 0` where the original's centre of gravity is exactly 0.
- **open, spec question 3 (betsurf on small images):** masks only (102): 3, 12, 18, 19, 28, 36,
  40, 53, 87, 89, 107, 112, 122, 140, 158, 161, 220, 239, 284, 295, 304, 306, 337, 344, 354, 355,
  386, 397, 446, 455, 512, 518, 531, 539, 556, 564, 566, 569, 579, 589, 620, 624, 641, 671, 700,
  747, 762, 776, 780, 812, 851, 856, 874, 892, 908, 941, 950, 957, 958, 959, 963, 966, 967, 976,
  994, 1000, 1008, 1034, 1052, 1057, 1059, 1069, 1101, 1104, 1122, 1278, 1306, 1308, 1355, 1375,
  1386, 1417, 1478, 1500, 1519, 1534, 1545, 1599, 1626, 1652, 1663, 1681, 1693, 1704, 1733, 1748,
  1758, 1771, 1779, 1801, 1805, 1818; the share of profiles set aside (12): 9, 114, 208, 301, 367,
  652, 893, 917, 1084, 1252, 1630, 1696; the vertex count with `-p` (10): 42, 261, 265, 557, 938,
  988, 1562, 1589, 1712, 1786.
- **open, spec question 4 (FSL-50's trigger):** 216, 635.
- **register, FSL-51 (undefined: searches past the samples):** 20, 124, 425, 610, 816, 1233.
- **timeouts (original):** bet2 52, 1028.

## 5. `docs/validation/golden.md`

After the part 2a sentence ("BET's clean-room original, part 2a (2026-10-30): ..."), add:

> BET's clean-room original, part 2b (2026-10-31): `cli.fsl.betsurf` (betsurf with a T2, `-o -m
> -s -v`, on a generated head with a generated sphere mesh and matrix), `cli.fsl.betsurf.t1only`
> (`--t1only -m -p 1`), `py.fsl.betsurf` (`lx.fsl.betsurf(implementation="original")`) and
> `cli.fsl.bet2.tanh` (`--tanh correctly-rounded`, GLIBC-06, the user's decision of 2026-10-31)
> recorded from ba0d924 (the parity smoke tier passing: 2,610 tests), every other checksum
> unchanged; `lx.fsl.betsurf` left `REPLICA_ONLY_FUNCTIONS`, now empty. The corrections of
> 2026-11-01 (`-c 0 0 0`, `bet -F`'s float64 `_tmp`) change no checksum.

In the "Command line, AFNI, MRI and FSL" row's BET list add: "`cli.fsl.bet2.tanh` (`--tanh
correctly-rounded`), `cli.fsl.betsurf` (T1 and T2, every output, verbose) and
`cli.fsl.betsurf.t1only` (`--t1only`, `-p 1`) (2026-10-31)", and in the Python row
`py.fsl.betsurf` (the original); the counts of those rows +3 and +1.

## 6. The site's shared files

- `data/status.yml`: regenerate from the merge with `python -m larmorx_validation tables --site
  <site>` (`validation/tools.toml`'s fsl-bet lines are updated in the branch; the row then says:
  "clean-room original (`bet2`, the `bet` script and `betsurf`), `experimental`; ... original:
  127 of 128 compared cases within thresholds against FSL 6.0.7.23, ...; betsurf's profiles decided
  as FSL's, its masks within its thresholds on 4 of 5 real heads (Dice 0.9907 to 0.9997); 2.5 to
  3.6 times faster than FSL on one thread, 6.5 to 13.5 times on 24, betsurf included; replica:
  ...").
- `content/validation.md`: no change needed (nothing there names BET's original).
- `content/roadmap.md`, row 1 ("BET's clean-room original"): move it to "Completed" with the date
  2026-10-31: "**BET's clean-room original** (`bet`, `bet2`, `betsurf`), beside the exact replica:
  every bet2 and bet case of the standard tier bit-identical to FSL 6.0.7.23 (fMRIPrep's calls
  included), betsurf's profiles decided as FSL's and its masks within their thresholds on 4 of 5
  real heads; 2.5 to 3.6 times FSL's speed on one thread. Open questions for its specification: a
  centre on neurologically stored images, betsurf where few profiles vote and on small images."
  The next step's status changes; "Last updated" 2026-11-01.
- `static/evidence/summary.json`: re-export (`python -m larmorx_validation evidence summary --site
  <site>`) after merging the branch.

## 7. Spec-writer questions (for `specs/bet.md`)

1. A centre's conversion between the voxel frames of a neurologically stored image (`bet2 -c`,
   `bet -R`'s centre of gravity): its last bits (cases 622, 677; the trial's 17 listed in §4; 965:
   FSL prints `-0.000001 0 0` where the exact centre is 0).
2. betsurf's deformation where the counting images are sparse (§6.5): which vertices are marked
   outside, the step where no gradient shows, the steps with no vote within reach (the inner
   skull up to 11 mm on a few vertices of ds002603's skull base).
3. The scalp phase (§6.5): its sphere, its mask and the small differences everywhere (0.2 to 3 mm
   at the 95th percentile on real heads; ds002603's scalp with its T2 misses its threshold).
4. §6.4 on small images whose profiles leave the image: what a sample beyond the image reads (the
   original reads 0 per corner outside `[0, n)`; three alternatives tried give none of FSL's
   shares on case 36).
5. FSL-50: the exact trigger (cases 216 and 635: FSL sets the profile aside without stopping).
6. §6.4's T1-only rule as observed: a profile is kept only when `OS2 >= IS` (64 of 64 triangles
   agree with it).
7. To add to §4.1 and §5.2 (observed, now followed): `-c 0 0 0` is "not given" (the centre of
   gravity, as without `-c`); `bet -F -d`'s `<out>_tmp` of a float64 series is float64 with the
   double values.

## 8. Commits

larmorx (worktree branch): b0c9284, f521d18, f1f9a35 (WIP: betsurf, the profile rules, `--tanh`), 095818a, 21762e8 (betsurf complete: probe knobs removed, memory), 9c29fe5 (PROVENANCE, findings, API), 1dd3492, c722f82, 5508182 (evidence), ba0d924 (investigations), 820e908 (golden), e45d805, bdbf2db, 9448976 (record, its summary), 2bd98f5 (benchmark, API, tables), 2950da2, e63df59 (the trial's two corrections), f847484 (record from e63df59); HEAD f847484. Site clone (`cleanroom-bet`): cf4804d (the page), 44ee1a7 (re-export from f847484); its
`evidence-blobs` branch (local, force-push it before `main`): 79a1750.

