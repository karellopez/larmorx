# 3dvolreg's replica part (2026-11-06 and 07): the handoff

The exact changes are in `HANDOFF.toml` beside this file (41 operations, 14 files: 11 of the main
repository, `validation/tools.toml` and `specs/3dTshift.md` among them, and 3 of the site). `python -P
scripts/orchestration/apply_handoff.py <scratch>/HANDOFF.toml --site <scratch>/site --check`
passes **against main's checkout** (33536e05) and the site clone (`volreg-pages`). Against the
part's own worktree (from fd3df7b, before PRELUDE's merge) five do not apply ([1], [2], [3], [5],
[22]: the register's counts, its gap line and PRELUDE's paragraph, and `docs/status.md`'s "Next",
whose text came with PRELUDE's merge): **apply the handoff after merging the branch into main**
(the `afni-volreg-replica` record of `validation/tools.toml` also needs the branch's record and
`tables.py`'s METRICS entry). If PRELUDE's clean-room handoff is applied first, the register's
counts ([1] to [4]), next-steps' order paragraph ([23]), `docs/status.md`'s "Next" ([22]) and the
roadmap's step 1 may no longer match: recount with `register_counts.py` and write them by hand.
Previewed after a scratch merge of the branch into main (the three conflicts resolved as below,
the handoff applied, `tables` and `evidence summary` run): `<scratch>/r6/shared-preview.diff` (the
main repository's 13 files) and `<scratch>/r6/site-shared-preview.diff` (the site's 4);
`register_counts.py` 421 entries ((a) 99, (b) 13, (c) 205, (d) 25, (e) 62, (f) 15; no decide, no
yes); `tables --check` passes; the previewed site builds with no problem but the evidence blobs the
light preview checkout lacks.

Afterwards: `python -m larmorx_validation tables --site <site>` (the README's tool table, the
validation index, the site's status block: 3dvolreg's new rows), `register_counts.py` (13 new
rows), `python -m
larmorx_validation evidence summary --site <site>` (3dvolreg's row: `evidence/summary.py`'s entry
is in the branch), and `python -m larmorx_validation evidence afni-volreg --site <site>` once more
from the merge commit (below, "AFNI-99's link").

Branch `worktree-agent-a1ad8a5fe6d84dce9` (worktree `larmorx/.claude/worktrees/agent-a1ad8a5fe6d84dce9`,
from main fd3df7b), 37 commits, the last `458c73e1` (tree clean). Site: branch `volreg-pages` of
`<scratch>/site` (from site main 72ca95e): `26311e7` (relay 5: 3dvolreg's page,
`content/tools/afni-volreg.md`, `static/evidence/afni-volreg.json`, the blobs' manifest) and
`9b5c556` (relay 6: the page after the record was made again, 190 cases, and AFNI-99's sentence);
`evidence-blobs` `b8cffa4` (1223 blobs: the 1209 before and 3dvolreg's 14; the manifest's SHA-256
745dddd6d7bde217). Push `evidence-blobs` (force) before `main`.

The site's checks (relay 6, after the re-export): build 0 problems (28 pages; 159 links into the
main repository checked against the scratch merge); 3dvolreg's page: number check 97 of 97 (relay 5's
page gives the same 97 of 97 in this machine's run), viewers 14 of 14, no failed example, 7.6 MB
(budget 40 MB). Inventory `<scratch>/r6/inventory.md` (main 72ca95e's build before, this branch
after): every page after >= before, 3dvolreg's page new (110 shown numbers, 3 viewers, 21 figures),
evidence files only added (the manifest gained lines, lost none). Against relay 5's page
(`<scratch>/r6/inventory-vs-relay5.md`) the tool marks the page "NO" only because four shown counts
grew with the record (189 -> 190 cases, 140 -> 141 bit-identical): the same 110 shown numbers, a row,
a link and 24 words added, nothing removed. Photographs `<scratch>/shots/r6/after/` (the agreement
chart, the record's paragraph, AFNI-99's paragraph; 1440 and 390, light and dark) and relay 5's
`<scratch>/shots/after/` (every section).

## Why each operation

- **The register, 13 rows (AFNI-86 to AFNI-99; AFNI-91 unused), all †:** found from AFNI 26.3.05's
  source while translating it (AFNI-97 by random testing, AFNI-98 and AFNI-99 by the spec writer);
  every behaviour checked against AFNI's binary where it can run (AFNI-96 only read: a dataset with
  `VOLREG_MATVEC_000000` and without its centres needs an AFNI extension written by hand). Classes:
  (a) AFNI-88, -89, -94, -96, -97 (crashes, a null pointer, an endless loop) and AFNI-90 (a buffer
  overflow; its second class (e) for `-maxdisp1D`'s cut), (c) AFNI-87, -92, -93, -99, (e) AFNI-86,
  -95, (f) AFNI-98. Gap **original** for all 13: the replica does it, 3dvolreg's clean-room original
  (the next part) is to do it from `specs/3dvolreg.md` §12. The counts' table and the line on second
  classes: the latter also gains FSL-268 and FSL-269 (PRELUDE's "c, a", missing since its merge:
  "Fifteen entries", recounted from the table).
- **The register's replicas' paragraph** (after PRELUDE's): what the replica corrects, refuses,
  keeps (rule C) and shares with 3dTshift.
- **The restricted section** (before FSL mcflirt's, with its own heading) and the blockquote at the
  top that lists the restricted sections (it gains 3dvolreg's and, harmonising, PRELUDE's
  `fsl-prelude.md`, which PRELUDE's handoff added to the heading only).
- **The user-facing table** (`docs/api/upstream-defects.md`): one row per correction with a switch
  (AFNI-86, -87, -90, -92, -93, -95, -97, -98, -99) after AFNI-75, and a paragraph for the four
  refused in both modes (as for AFNI-40 and AFNI-46).
- **CLAUDE.md rule 1 and the porting guide §1:** `docs/findings/afni-volreg.md` is source-derived.
- **`validation/tools.toml`:** 3dvolreg's `[[tool]]` (after 3dUnifize) and `[[record]]` (after
  3dUnifize's replica); `tables.py`'s METRICS has `afni-volreg-replica` (the AFNI replicas' counts)
  in the branch. The generated rows (previewed): the README's tool table "replica: [137 of 137](docs/validation/afni-volreg-replica.md) compared cases that write outputs bit-identical to AFNI 26.3.05 ...; 30 more rejected by both alike; 2.2 to 3.1 times faster than AFNI (informal timings)", the validation index's `afni-volreg-replica` row (`validated`), the site's status block.
- **`docs/validation/golden.md`:** the AFNI replicas' counts (52 cases, 129 outputs), the main
  package's (237 and 1,728: `py.afni.fmriprep_volreg_options`), the `lx.afni` row, 3dvolreg's row
  in the replicas' table and its paragraph (text outputs fingerprinted; the recordings; AFNI-99's
  exact notice changing no checksum).
- **`docs/licensing.md`:** EISPACK's `svd` and `pythag` and libf2c's `d_sign`, which 3dvolreg's SVD
  translates (a new upstream row: EISPACK freely available, libf2c under AT&T Bell Laboratories and
  Bellcore's permission notice, now in `NOTICE`), and the 22 AFNI files 3dvolreg added to the 55 of
  the relicensing check, every header read: 13 MCW (CC BY 4.0), 6 NIH (no header), 3 EISPACK/f2c;
  none GPL.
- **`docs/status.md`:** 3dvolreg's replica in the AFNI row and the FSL paragraph's "Next".
- **`docs/next-steps.md`:** the order paragraph's item (4) done; a follow-ups block (the clean-room
  part's inputs and closed folders, 3dTshift's answered weights, the AFNI tools' shared items, the
  clean-room AFNI crate's long output names, the final phase's items); the spec's open point under
  "Waiting for the user" (below).
- **`docs/validation/difftest.md`:** 3dvolreg's row in the tools table.
- **`specs/3dTshift.md` §5.2:** its open point (the quintic and heptic weights' last bits) answered
  by this part's spec writer: 3dTshift and 3dvolreg shift rows with the same AFNI arithmetic, and
  the weights are AFNI's bit for bit when `x²` is a float32 product and the rest double in the
  factored forms of `specs/3dvolreg.md` §5.6, the constant last (checked by 3dvolreg's model). No
  routine names or code in the text.
- **The site:** `site.yml` (3dvolreg after 3dUnifize; the three AFNI entries no longer say the
  replica is in `larmorx-gpl`: in `larmorx` since 2026-10-29), `data/status.yml` (AFNI's planned
  line; the generated block from `tables --site`), `content/roadmap.md` (Last updated, a step for
  3dvolreg after step 1 with the later ones renumbered, a Completed row),
  `static/evidence/summary.json` (re-export).

## For the orchestrator (not in the operations)

### The expected merge conflicts (seen merging the branch into main 33536e05 in a scratch clone)

- `tests/golden/golden.json`: only the `recorded` header (main 1eb168ff, the branch 1341c6e2); the
  cases merge (237, `py.afni.fmriprep_volreg_options` added). Take either header, then
  `scripts/record_golden.py` from the merge commit (verify-merge does): every checksum unchanged.
- `tests/golden/registry.py`: `REPLICA_ONLY_FUNCTIONS` and its comment. Keep both entries (unless
  PRELUDE's clean-room original, merged first, has removed `lx.fsl.prelude`):

      #: below). Since 2026-11-06 ``lx.fsl.prelude`` runs a tool with only its replica (FSL's
      #: prelude), and ``lx.afni.volreg`` (AFNI's 3dvolreg).
      REPLICA_ONLY_FUNCTIONS: dict[str, str] = {
          "lx.fsl.prelude": "fsl prelude",
          "lx.afni.volreg": "afni 3dvolreg",
      }

- `validation/difftest/triage.toml`: both sides appended at the end; keep main's (prelude's) then
  the branch's (volreg's 8 entries), and check it with `tomllib`.

No other file conflicts (`crates/larmorx-core/src/defects.rs`, `NOTICE`, the AFNI replica crate,
`validation/src/larmorx_validation/__main__.py`, `difftest/tools/__init__.py`,
`evidence/__init__.py`, `tables.py` merge cleanly). The shared harness changes to review at the
merge: `parity/afni_replica.py` (text outputs, `derive`, `every_file`, `unnamed_ok`,
`fmriprep_build`, AFNI's text kept before a notice, glibc's abort line in `check_crash`),
`difftest/runner.py` (AFNI's "Fatal Signal" banner), `difftest/permanent.py` (`fix_writes`).

### AFNI-99's link: the export to run again after the merge

The site page's corrections paragraph, its AFNI-99 example, the defect fact and the series
example's differences link AFNI-99 to `docs/api/afni-volreg.md#upstream-defects` while the register
has no AFNI-99 row (the branch). After the handoff is applied, `python -m larmorx_validation
evidence afni-volreg --site <site>` from the merge commit takes the register's anchor (the module's
`_link`); the figures cache serves every run (no AFNI or replica run again); then the site's checks
and `evidence summary`. The two links of the page's text to the API page's table stay.

### The clean-room audit's tables for 3dvolreg's clean-room part

In main's `validation/src/larmorx_validation/cleanroom_audit/` (not in this branch, which predates
the audit's merge):
- `restrictions.py`: `SOURCE_FINDINGS["afni-volreg.md"] = ("AFNI", "2026-11-06T22:01:36+01:00",
  <the merge commit that lists it in CLAUDE.md rule 1>)` (its first commit, 127c7d83).
- `verdicts.py`: `tool_of`'s AFNI loop `for t in ("tshift", "automask", "unifize", "volreg")` (the
  clean-room crate's `volreg.rs` or `volreg/`); `TOOL_NAMES[("AFNI", "volreg")] = "3dvolreg"`;
  `BRIEF_FILES` += `afni-volreg` (the replica author's investigation folder
  `validation/investigations/afni-volreg/`; not `3dvolreg-spec`, the spec writer's model written
  from the spec and black-box runs, open); `BRIEF_RESTRICTED` += `afni_volreg` (the replica's
  `parity/afni_volreg.py`, `difftest/tools/afni_volreg.py` and `evidence/afni_volreg.py`, written by
  the replica author; unlike PRELUDE's, their names carry no `replica`, so a clean-room suite needs
  another name, e.g. `parity/afni_volreg_original.py`): `(?:\w*replica\w*|fsl_mcflirt\w*|afni_volreg)\.py$`.
  The permanent cases (`validation/difftest/cases/afni-volreg/`, 7) are R15 already, the probe
  `validation/probes/afni-volreg-trace/` R14 (family by its name).
- `similarity.py`: `AFNI_MODULE_TOOLS` += `vecmat`, `eispack`, `shear`, `rot3d`, `blur`, `lsqfit`,
  `align3d`, `volreg`, `cli::volreg` → `"volreg"` (the replica's modules). The upstream files follow
  from the crate's `PROVENANCE.md` (`afni_upstream_tools`): 16 become 3dvolreg's (`thd_shear3d.c`,
  `.h`, `mri_lsqfit.c`, `eispack/eis_svd.c`, `f2c/d_sign.c`, `cs_symeig.c`, `thd_rotangles.c`,
  `thd_center.c`, `thd_mismatch.c`, `thd_read_vecmat.c`, `thd_tshift.c`, `thd_zeropad.c`,
  `cs_addto_args.c`, `mri_to_byte.c`, `mri_to_short.c`, `mri_allzero.c`), but `3dvolreg.c`,
  `mri_3dalign.c`, `thd_rot3d.c` and `edt_blur.c` become shared code because the `environ` row
  names them (3dvolreg's environment variables are read there): an override (e.g.
  `AFNI_FILE_TOOLS = {"src/3dvolreg.c": "volreg", "src/mri_3dalign.c": "volreg", "src/thd_rot3d.c":
  "volreg", "src/edt_blur.c": "volreg"}`) counts them as 3dvolreg's; dropping `environ`'s rows
  instead would move `afni_environ.c` to 3dTshift. `shift` (`thd_shift2.c`) stays 3dTshift's, though
  3dvolreg shifts its rows with it too (the orchestrator may make it shared). The similarity
  re-measured with 3dvolreg's upstream at the clean-room part's merge (next-steps, the audit's
  item (c)).
- Transcripts to archive (NOTES.md has them): relays 1 to 6, `agent-a1ad8a5fe6d84dce9`,
  `agent-a1cf390e6ff9d69b7`, `agent-a9ebe8c3189e65069`, `agent-a03531d54165580fa`,
  `agent-ad17fb9f7ee53ae56`, `agent-abc04e3d9acb2f4fd` (session b322ad6c-...). Every one a replica
  author and spec writer who read AFNI's source or the replica; none touched a clean-room crate.

### The spec's open question (`specs/3dvolreg.md` §16), for the user's batch

AFNI computes the four shears of a rotation with a generated closed form whose last double bits
decide the polynomial methods' last bits (for a rotation about one axis, every derivative image of
the fit, four of the six axis orders tie). The spec gives the mathematics (the stable formulas) and
the clean-room original is validated within thresholds: the spec's model, written from the spec,
lands within 0.0012 (degrees or mm) of AFNI's parameters and 2.7e-5 of the largest value in the
volumes, and is bit-identical once AFNI's decomposition values are plugged in. Describing AFNI's own
evaluation, operation by operation, would make the original bit-identical on the polynomial
methods, but that is AFNI's code put into the spec. **Recommendation: no** (rule C-like: the replica
already gives AFNI's bits for whoever needs them; the difference is a fraction of 3dvolreg's own
convergence threshold). The operation [25] puts it under "Waiting for the user".

### A behaviour-only bug report for the clean-room AFNI crate (relay 3)

"Output names of about 228 to 255 bytes cannot be written." AFNI writes an output whose name is up
to 255 bytes (from 256 it aborts, AFNI-90). The program first writes a temporary file whose name (a
hidden prefix, the output's name, a process number and a counter) is longer than the system's
255-byte limit for one file name, so a `-prefix` (or `-savedisp`) of about 228 to 255 bytes fails
with "File name too long" where AFNI writes it. The replica's temporary names were cut to 255 bytes
(2026-11-06, 1aa88000); the clean-room AFNI crate (`crates/larmorx-afni`, every clean-room AFNI
tool) names its temporary files the same way and may share it: test a 240-byte `-prefix` there.
(Reported from the black-box behaviour only; the next-steps follow-up says so.)

### Smaller notes

- The final check found AFNI-99's notice imprecise (relay 6; the report): fixed, with no output
  changed, before the record was made again. The smoke tier last passed at 1341c6e2 (relay 5);
  since then the AFNI replica's Rust changed only in AFNI-99's notice and comments (and the core
  registry's AFNI-90 text): the 3dvolreg cases of `pytest tests/parity` were run again
  at the final code (`test_afni_volreg_replica.py`: 42 passed; `test_memory_peak.py -k volreg`:
  passed); verify-merge runs the whole tier.
- The register's text for AFNI-90 and the core registry now agree: `-maxdisp1D`'s name keeps its
  first 997 bytes (the registry said "cut at 998 bytes", `MCW_strncpy`'s size; corrected in the
  branch). NOTICE gained 3dvolreg and EISPACK with libf2c's permission notice (`d_sign`); its copy
  `crates-nc/LICENSES/NOTICE-larmorx.txt` follows (the `notices` test).
- The site's home page cards are older than the AFNI move (AFNI: "3dTshift ... with an exact
  replica in `larmorx-gpl`"; FSL: the head-motion correction only; the status lede names AFNI
  25.2.09 and FSL 6.0.7): left as they are (the landing page's text is the user's design), a site
  follow-up.
- `ORCHESTRATOR-STATE.md` and CLAUDE.md's "Where things are": the orchestrator's own text.
- `docs/parts/<the part>/`: BRIEF.md, NOTES.md (the six relays' agent ids), this file, HANDOFF.toml
  and the final report.

## Tell the user (through the orchestrator)

- 3dvolreg (AFNI's motion correction, which fMRIPrep runs to build every BOLD reference) has its
  exact replica: every file AFNI writes, bit for bit, in every compared case, fMRIPrep's real calls
  included (and fMRIPrep's own BOLD references come out the same), and about 2 to 3 times faster
  than AFNI.
- Thirteen defects of AFNI's 3dvolreg found (AFNI-86 to AFNI-99), nine corrected by default with
  AFNI's behaviour on request, four crashes refused. The one that touches real data: the "maximum
  displacement" numbers AFNI prints are computed for mirrored positions on most NIfTI files (RAS or
  LAS storage); fMRIPrep writes them but does not use them, so its results are not affected.
- The behavioural specification for 3dvolreg's clean-room original is written and checked by a
  model against AFNI. One question for the user's batch: whether the clean-room original should be
  made bit-identical on one numerical detail by describing AFNI's own arithmetic in the spec
  (recommendation: no).
