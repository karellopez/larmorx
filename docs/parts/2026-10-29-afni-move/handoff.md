# HANDOFF: the AFNI replicas move into the main package (next-steps item 3c), 2026-10-29/30

Branch: the worktree agent-ac71aae39a4bfa297 (from main 57ab243; last commit e55138c). Site:
<scratch>/site, branch `afni-move` (commit d6c1d24). Every block below is the exact text for a shared document; "replace" quotes the
current text first. Defect IDs used: none (AFNI-76 to AFNI-80 unused: no new upstream defect).

## 1. CLAUDE.md

1a. "Where things are", the AFNI row: replace
"clean-room originals (`crates/larmorx-afni`) and GPL replicas (`crates-gpl/`), both validated against AFNI 26.3.05 since 2026-10-29"
with
"clean-room originals (`crates/larmorx-afni`, built for speed) and replicas (`crates/larmorx-afni-replica`, in the main package since 2026-10-29: Apache-2.0 with CC BY 4.0's attribution, run in-process; `implementation="auto"` runs them), both validated against AFNI 26.3.05 since 2026-10-29"

1b. "Where things are", "Next:" paragraph: replace
"Next: the
AFNI replicas moved into the main package (user, 2026-10-29; next-steps item 3c), then BET's clean-room original"
with
"The AFNI replicas moved into the main package the same day (next-steps item 3c: `sinf`/`cosf`
from Arm's MIT routines proved equal to glibc's; `cbrtf`'s last bit measured, a decision for the
user). Next: BET's clean-room original"

1c. "The user's direction", the bullet "**AFNI from 26.3.05**": replace
"so the AFNI replicas move into the main package (next-steps item 3c: CC BY 4.0's attribution, `sinf`/`cosf` from Arm's MIT routines, `cbrtf` measured first); the clean-room originals stay, built for speed."
with
"so the AFNI replicas moved into the main package (2026-10-29, next-steps item 3c: CC BY 4.0's attribution, `sinf`/`cosf` from Arm's MIT routines, `cbrtf` measured: its last bit matters, the user decides how to keep it exact); the clean-room originals stay, built for speed."

1d. Rule 1, the replica tracks: replace
"     - AFNI's MCW files and Workbench (GPL-2.0-or-later): `larmorx-gpl` (GPL-3.0-or-later, `crates-gpl/`); AFNI's MCW files are CC BY 4.0 from AFNI 26.3.05, and the AFNI replicas move into the main package (user, 2026-10-29; next-steps item 3c), until then they stay here;"
with
"     - AFNI's MCW files: CC BY 4.0 from AFNI 26.3.05 (GPL-2.0-or-later until 25.2.09): the main package since 2026-10-29 (`crates/larmorx-afni-replica`, Apache-2.0 with CC BY 4.0's attribution in each file's header, its `PROVENANCE.md` and `NOTICE`; the legal code in `LICENSES/`);
     - Workbench (GPL-2.0-or-later): `larmorx-gpl` (GPL-3.0-or-later, `crates-gpl/`, which today holds only the verification tools that keep glibc's LGPL code);"
and in the same list's first item add AFNI's MCW files after "AFNI's NIH files": "permissive upstreams (ANTs/ITK, AFNI's NIH files, AFNI's MCW files from 26.3.05 (CC BY 4.0), Arm's Optimized Routines, CORE-MATH, nibabel): the main `larmorx` package (Apache-2.0);"

1e. Rule 1, "Original": in the list of what the implementer never reads, replace
"never the upstream source, the replica crates or source-derived findings"
with
"never the upstream source, the replica crates (`crates/larmorx-afni-replica` since 2026-10-29, `crates-gpl/tools/glibc-reference`, `crates-nc/`) or source-derived findings"

1f. Rule 1: replace
"   - **Never** move replica code into the main package, even renamed."
with
"   - **Never** move replica code into the main package, even renamed, unless the upstream itself releases it under a licence the main package allows and the user decides the move (AFNI's MCW code, CC BY 4.0, 2026-10-29)."

1g. Rule 2, add after the AFNI line:
"   - Arm's Optimized Routines v19.11 (the AFNI replicas' `sinf`/`cosf`, the code glibc 2.35 ships; MIT)"

1h. Repo map: the `Cargo.toml, crates/` row: replace "`-afni` (clean-room), `-mri` (clean-room)" with "`-afni` (clean-room), `-afni-replica` (the AFNI replicas, CC BY 4.0's attribution), `-mri` (clean-room)"; the `crates-gpl/` row: replace its text with
"the GPL-3.0-or-later workspace, reserved for Workbench's replicas; since 2026-10-29 it holds only the verification tools that keep the AFNI replicas' earlier glibc translations (`tools/glibc-reference`, `sincosf-verify`, `floatmath-verify`, `cbrtf-measure`); it builds the Apache crates by path, so refresh `crates-gpl/Cargo.lock` whenever their dependencies change; CI does not build it"

## 2. docs/licensing.md

2a. "Upstream by upstream", the MCW row: replace
"| AFNI, files copyrighted by the Medical College of Wisconsin | GPL \"Version 2 (or any later edition)\" (`doc/README/README.copyright`) | ✓ `larmorx-gpl`, as GPL-3.0-or-later | ✓ main package |"
with
"| AFNI, files copyrighted by the Medical College of Wisconsin | CC BY 4.0 from AFNI 26.3.05 (\"As of May 12, 2026, MCW has granted NIH a Creative Commons Attribution License (CC BY 4.0) for all of the code under its copyright\", `doc/README/README.copyright`); GPL \"Version 2 (or any later edition)\" until 25.2.09 | ✓ main package since 2026-10-29 (`crates/larmorx-afni-replica`, with CC BY 4.0's attribution; `larmorx-gpl` until then) | ✓ main package (built for speed) |"
and add a row after CORE-MATH's:
"| Arm's Optimized Routines (v19.11: the `sinf`/`cosf` glibc 2.35 ships) | MIT | ✓ main package (the AFNI replicas' `sincosf.rs`) | not needed |"

2b. "Packages" table, the `larmorx-gpl` row: replace "replicas of AFNI's MCW files and of Workbench" with "replicas of Workbench (none yet; until 2026-10-29 also AFNI's MCW files)".

2c. Replace the bullet "**A replica package is a program, not a Python module** (decided 2026-10-11). `larmorx-gpl` is a binary-only wheel ... checks the copies and the source manifest." first sentences so that it begins:
"- **A replica package is a program, not a Python module** (decided 2026-10-11). `larmorx-nc` is a binary-only wheel (maturin's `bin` bindings, `crates-nc/pyproject.toml`, licence `LicenseRef-FSL`): it installs the `larmorx-nc` program into the environment's scripts directory and nothing that Python can import. `larmorx-gpl` was packaged the same way for the AFNI replicas until 2026-10-29 (its wheel, program and `larmorx[exact]` extra are in git history); it will be again for Workbench."
then keep the `larmorx-nc` sentences that follow ("its wheel carries the FSL Licence ... checks the copies and the source manifest.").

2d. "Nothing is on PyPI yet" block: remove the two lines
"  (cd crates-gpl && maturin build --release --locked --out ../dist)   # larmorx-gpl"
"  pip install --find-links dist \"larmorx[exact]\"                      # + the GPL replicas"
and change the comment of the first line to "# larmorx (repository root): the originals and the AFNI replicas".

2e. "Notes checked 2026-10-19", replace the paragraph beginning "3dUnifize's squash also calls the C library's `tanhf`" (to "... `afni-unifize.md`.") with:
"3dUnifize's squash also calls the C library's `tanhf`, and `-EPI` its `cbrtf`. Since 2026-10-29 the
replica (in the main package) carries fdlibm's `tanhf` (Sun's notice), checked against glibc on all
2^32 float inputs, and computes `-EPI`'s cube root correctly rounded (CORE-MATH, MIT): glibc's own
`cbrtf` is LGPL-2.1 code, and its last bit changes the ball of neighbours on some voxel volumes, so
the replica warns there (`docs/findings/afni-relicensing-26.md`, "The C library's maths"; the
user decides how to keep the exact result). `specs/3dAutomask.md` and `specs/3dUnifize.md` are the
clean-room originals' input. The routine-by-routine tables, with file:line, are in the restricted
findings `docs/findings/afni-automask.md` and `afni-unifize.md`."

2f. Replace the bullet "**AFNI 26.3.05 relicenses MCW's code** (checked 2026-10-29, ... carries the CC BY 4.0 attribution." with:
"- **AFNI 26.3.05 relicenses MCW's code** (checked 2026-10-29,
  [findings/afni-relicensing-26.md](findings/afni-relicensing-26.md)): \"As of May 12, 2026, MCW
  has granted NIH a Creative Commons Attribution License (CC BY 4.0) for all of the code under
  its copyright\" (`doc/README/README.copyright`); every MCW file's header now says CC BY 4.0, and
  none of the 55 AFNI files the replicas derive from is GPL. **The AFNI replicas moved into the
  main package on 2026-10-29** (decided by the user): `crates/larmorx-afni-replica`, Apache-2.0,
  with CC BY 4.0's attribution (section 3(a): the creator, the copyright notice, the licence and
  its URI, the disclaimer, the source and its tag, the modification) in every MCW-derived file's
  header, in the crate's `PROVENANCE.md` and in `NOTICE`, the legal code in `LICENSES/CC-BY-4.0.txt`
  (CC BY 4.0 lets an adaptation carry the adapter's licence, section 3(a)(4)). The C library's
  functions are permissive: `sinf`/`cosf` translated from Arm's Optimized Routines (MIT,
  `LICENSES/MIT-arm-optimized-routines.txt`), `tanhf`/`expm1f` fdlibm's; glibc's LGPL `cbrtf` is
  not included (above). larmorx's own code written for `larmorx-gpl` was relicensed to Apache-2.0
  by its author. `scripts/check_spdx.py` allows CC BY 4.0 only in that crate."

2g. "Choosing at run time": replace the first paragraph
"**The licences stay separate through process boundaries** (decided 2026-10-11). The Apache package `larmorx` never imports, loads or links replica code, in Rust or in Python."
with
"**The licences stay separate through process boundaries** (decided 2026-10-11). The Apache package `larmorx` never imports, loads or links the code of a replica under another licence family (`larmorx-nc`), in Rust or in Python. **A replica under a licence the main package allows is built in** (the AFNI replicas since 2026-10-29): it runs in the same process, with the arguments, directory and environment a separate program would get, and gives the same files, messages and exit status."
and in its "Python" bullets replace
"    - `\"auto\"` (the default) runs the replica when its program is found, and the original
    otherwise; `\"replica\"` and `\"original\"` force one."
with
"    - `\"auto\"` (the default) runs the replica when it is built in (AFNI's) or its program is
    found (FSL's), and the original otherwise; `\"replica\"` and `\"original\"` force one."
and "    - `\"replica\"` without the program raises `lx.ReplicaNotFoundError`" with "    - `\"replica\"` without a separate replica's program raises `lx.ReplicaNotFoundError`";
"    - The wrapper writes in-memory inputs to a temporary directory, runs the program there and reads its output back." with "    - The wrapper writes in-memory inputs to a temporary directory, runs the replica there (a built-in one in this process) and reads its output back."
In "Finding the program": replace "`LARMORX_GPL_BIN` or `LARMORX_NC_BIN`" with "`LARMORX_NC_BIN`".
In "Installing the replicas is explicit": replace the sentence "`pip install \"larmorx[exact]\"` adds `larmorx-gpl`; `larmorx-nc` (non-commercial) comes only by name or with `pip install \"larmorx[all]\"`, which installs everything (the user's decision of 2026-10-16), so its licence is accepted knowingly either way; `[exact]` never brings it." with "The AFNI replicas come with `pip install larmorx`; `larmorx-nc` (non-commercial) comes only by name or with `pip install \"larmorx[all]\"`, which installs everything (the user's decision of 2026-10-16), so its licence is accepted knowingly either way."

## 3. docs/overview.md

- Line 45, the MCW row: "| AFNI, files copyrighted by the Medical College of Wisconsin (e.g. 3dTshift) | CC BY 4.0 from AFNI 26.3.05 (GPL-2.0-or-later before) | main package (since 2026-10-29) | main package |"
- Line 64: replace `pip install "larmorx[exact]"         # + larmorx-gpl: exact AFNI/Workbench replicas` with `pip install larmorx                  # includes the exact AFNI replicas (since 2026-10-29)`.
- Lines 78-79: replace "a separate process and exchanges files with it, so installing `larmorx[exact]` does not put GPL code into your Python program" with "a separate process and exchanges files with it, so installing `larmorx-nc` does not put FSL-licensed code into your Python program; the AFNI replicas, permissive since AFNI 26.3.05, are part of larmorx and run in its process".
- Line 81: replace `pip install --find-links dist "larmorx[exact]"` with `pip install --find-links dist larmorx`.
- Lines 97-98: replace "GPL replicas in `crates-gpl/`, with the GPL text, the upstream copyright notices, and changes marked." with "GPL replicas (Workbench's, to come) in `crates-gpl/`, with the GPL text, the upstream copyright notices, and changes marked; it holds only verification tools today."

## 4. README.md

- Lines 41-44: in "where the licence is restrictive (AFNI's GPL code, FSL), a **clean-room original** is ... upstream's licence (e.g. `larmorx-gpl`) that runs as a separate program." replace "AFNI's GPL code, FSL" with "FSL, Workbench", "(e.g. `larmorx-gpl`)" with "(e.g. `larmorx-nc`)", and add after the sentence: "AFNI's tools have both inside larmorx since AFNI 26.3.05 released its MCW code under CC BY 4.0."
- Line 81: replace `pip install "larmorx[exact]"         # + exact replicas of GPL-licensed tools       (planned)` with `pip install larmorx                  # the exact AFNI replicas included (CC BY 4.0's attribution)`.
- Lines 129-131 are generated from `validation/tools.toml` (section 12).
- Line 192: replace "The GPL replicas are a separate Cargo workspace: `cd crates-gpl && cargo test --workspace`" with "The FSL replicas are a separate Cargo workspace: `cd crates-nc && cargo test --workspace`; `crates-gpl` holds only verification tools (`crates-gpl/README.md`)".
- Line 212: replace the `crates-gpl/` row's text with "the GPL-3.0-or-later workspace, reserved for Workbench's replicas; today the AFNI replicas' verification tools (`crates-gpl/README.md`)", and add a row "| `crates/larmorx-afni-replica` | the AFNI replicas (3dTshift, 3dAutomask, 3dUnifize), Apache-2.0 with CC BY 4.0's attribution |".
- Line 226: replace "under their upstream's licence (`larmorx-gpl`: GPL-3.0-or-later)" with "under their upstream's licence (`larmorx-nc`: the FSL Licence)" and add: "The AFNI replicas carry CC BY 4.0's attribution for the Medical College of Wisconsin's code ([NOTICE](NOTICE), `LICENSES/`)."

## 5. UPSTREAM.md

- The AFNI row's last column: replace "the GPL replicas (`crates-gpl/`, GPL-3.0-or-later) follow 26.3.05" with "the replicas (`crates/larmorx-afni-replica`, in the main package since 2026-10-29, Apache-2.0 with CC BY 4.0's attribution) follow 26.3.05", and "(the move into the main package decided by the user on 2026-10-29, next-steps item 3c)" with "(the move into the main package done on 2026-10-29, next-steps item 3c)".
- A new row: "| Arm Optimized Routines | MIT | `v19.11` (`math/sincosf.h`, `sinf.c`, `cosf.c`, `sincosf_data.c`, the files glibc 2.35 ships; fetched by `validation/investigations/afni-move/fetch_aor.sh`) | v19.11 | the AFNI replicas' `sinf`/`cosf` (`crates/larmorx-afni-replica/src/sincosf.rs`) |"
  (and in upstream.tsv, if it lists non-cloned sources: `optimized-routines	https://github.com/ARM-software/optimized-routines	v19.11	MIT	fetched`; the commit is not recorded: the files were fetched by tag, their SHA-256 in `crates/larmorx-afni-replica/PROVENANCE.md` is not recorded either: cosf.c 9c83b298…, sinf.c 06f1749d…, sincosf.h 15a25443…, sincosf_data.c 49ddccfd…).

## 6. docs/porting-guide.md §1

- The table's row "| AFNI's MCW files, Workbench (GPL) | `larmorx-gpl` (`crates-gpl/`) | main package, from a spec |" becomes two rows:
  "| AFNI's MCW files (CC BY 4.0 from 26.3.05; GPL-2 before) | main package since 2026-10-29 (`crates/larmorx-afni-replica`), with CC BY 4.0's attribution | main package, from a spec (built for speed) |"
  "| Workbench (GPL) | `larmorx-gpl` (`crates-gpl/`) | main package, from a spec |"
- In "The clean-room implementer must not open the upstream source, the replica crates, or findings written from the source:", replace "the replica crates" with "the replica crates (`crates/larmorx-afni-replica`, `crates-gpl/tools/glibc-reference`, `crates-nc/`)".
- In "Why both tracks for restricted upstreams": replace "`implementation=\"auto\"` runs the replica when it is installed, as a separate program, never imported" with "`implementation=\"auto\"` runs the replica: a separate program, never imported, for a restricted licence (FSL); built in for AFNI since its code became CC BY 4.0".

## 7. The register (docs/findings/upstream-defects.md) and docs/api/upstream-defects.md

No new defect, no status change (AFNI-76 to AFNI-80 unused). Where the AFNI rows say "the GPL replica" or `larmorx-gpl`, the harmonization may read "the replica (in the main package since 2026-10-29)".

## 8. docs/next-steps.md

8a. Item 3c: replace its text with
"   - **3c. The AFNI replicas move into the main package: done** (2026-10-29/30,
     `docs/parts/2026-10-29-afni-move/`, [findings](findings/afni-relicensing-26.md), \"The move\"):
     `crates/larmorx-afni-replica` (Apache-2.0 with CC BY 4.0's attribution per file, in
     `PROVENANCE.md`, `NOTICE`; the legal code and Arm's MIT licence in `LICENSES/`), run in-process
     by the `larmorx` command line and the Python wrappers; `sinf`/`cosf` translated afresh from Arm's
     Optimized Routines v19.11 and proved equal to glibc's on all 2^32 inputs of each function and
     build (FMA, SSE2); `tanhf` fdlibm's; `cbrtf` measured: its last bit changes 3dUnifize `-EPI`'s
     ball on 1,449,010 float volumes (80 in [1, 100) mm³), so the replica takes CORE-MATH's and
     warns where it could matter (the user decides, \"Waiting for the user\"); the three replica
     records re-made in-process: no status change; golden checksums unchanged; `crates-gpl/` keeps
     only the verification tools; CI's `crates-gpl` steps gone (4.0 min a run)."

8b. Follow-ups (add):
"- **3dUnifize's replica memory** (found 2026-10-29, the move's memory cases): its peak is 1.5
  (`-T2`, 160×192×160) and 2.8 (`-EPI`, 64×64×40×160) times AFNI 26.3.05's, as it was as the
  `larmorx-gpl` program (`validation/investigations/afni-move/memory.py`); a replica-memory part
  like antsAI's (no full-size copies AFNI does not hold). Strict expected failures in
  `tests/parity/test_memory_peak.py` until then.
- **The Python entry point's own memory** (2026-10-29): `python -m larmorx` takes about 13 MiB more
  than the binary (the interpreter and numpy), beyond the rule's 8 MiB on a small image
  (3dAutomask on a 64×64×40 mean: 21.2 MiB against AFNI's 7.5; the replica alone 8.2): a lazy
  numpy import on the command-line path would bring it within. A strict expected failure until then.
- **The AFNI evidence pages' entries** (2026-10-29): the figures cache's AFNI entries were computed
  with the `larmorx-gpl` program; their replica runs could be recomputed in-process at the final
  validation phase (the records show the same files; the computes now run the in-process replica).
- **CI's `gpl` job name** (2026-10-29): it runs only `crates-nc` now; renaming it (`nc`) would change
  the check names the watchers count (kept as is)."

8c. "Waiting for the user" (add):
"- **3dUnifize `-EPI`'s `cbrtf`** (2026-10-29, the AFNI move; [findings](findings/afni-relicensing-26.md),
  \"The C library's maths\"): glibc's `cbrtf` (LGPL, not in larmorx) and the correctly rounded one
  the replica now uses differ in the last bit on one input in ten; on 8,794 voxel volumes from
  0.001 mm³ (80 in [1, 100) mm³) that bit changes `-EPI`'s ball of neighbours, so the replica is
  not AFNI's there and warns (on 2,953 volumes in [1, 100) mm³, every changed one among them).
  **Recommended:** a clean-room `cbrtf` with glibc's bits (a spec of the method by a spec writer,
  written by an agent that has not read glibc, proved equal on all 2^32 inputs; a small part).
  Alternatives: a table of the 9,346 changed volumes from 0.001 mm³ (exact on realistic voxels
  only), or keeping the warning."
"- **For the user's information: `auto` now runs the AFNI replicas** (built in), and the replicas'
  speed against the clean-room originals on the standard tiers' real cases is in
  `validation/investigations/afni-move/README.md`: the clean-room originals are faster (the
  original's time over the replica's, median of the real cases: 3dTshift 0.95 at one thread and
  0.72 at eight, 3dAutomask 0.54 and 0.44, 3dUnifize 0.47 and 0.27); the rule (the replica when
  available) is unchanged; the user may want to revisit the default."

## 9. docs/decision-log.md (new entry)

"## 2026-10-29: The AFNI replicas in the main package (the move, item 3c; rules and measurements)
- **Done as decided by the user:** the replicas are `crates/larmorx-afni-replica`, Apache-2.0 with
  CC BY 4.0's attribution, built into larmorx (in-process; `implementation=` unchanged, `auto`
  keeps its rule and so runs them). `larmorx-gpl` keeps no package; `crates-gpl/` holds the GPL
  verification tools with the earlier glibc translations (never distributed). The `larmorx[exact]`
  extra and the `larmorx-gpl` wheel are gone (nothing was on PyPI: no alias).
- **The C library's functions (rule D for `cbrtf`):** `sinf`/`cosf` from Arm's MIT routines, proved
  equal on all 2^32 inputs of each function and build; `tanhf`/`expm1f` fdlibm's; `cbrtf` measured:
  its last bit matters (1,449,010 balls changed over the float volumes; 80 in [1, 100) mm³), so
  the decision goes to the user (\"Waiting for the user\"); meanwhile the replica uses CORE-MATH's and
  warns wherever one unit in the last place could change the result (0 changed volumes unwarned).
- **By the rules:** larmorx's own code written for `larmorx-gpl` relicensed to Apache-2.0 by its
  author; the replica's messages name `larmorx` instead of `larmorx-gpl`; the warning ends in
  `(larmorx)`, so the parity harness keeps it out of AFNI's messages; CI's `crates-gpl` steps
  removed (no package code left), the `gpl` job kept for `crates-nc`.
- **Memory:** the move adds none (the in-process replica within 1.2 MiB of the separate program);
  the replica's 3dUnifize and the Python entry point on small images exceed the rule as before:
  follow-ups, strict expected failures."

## 10. validation/difftest/triage.toml

No entry. The trial (2026-10-30, 30 minutes, from 6ee7765, `<workspace>/oracles/difftest/2026-10-30-afni-move/`):
13,971 cases, no difference; D13 notices of the known classes (AFNI-01, -16, -43, -45), AFNI-20's
random identifier (44), 14 3dUnifize `-EPI` cases queued as heavy (AFNI-75's class). Two earlier
attempts set aside (`...-invalid-extension-replaced/`, `...-contaminated-overlap/`; README).

## 11. docs/validation/golden.md

- "Where they are" table: the `larmorx-gpl` column becomes "the AFNI replicas (in `larmorx`)":
  Cases `crates/larmorx-cli/tests/afni_replica_golden.rs`: 40 cases, 90 outputs; Checksums
  `crates/larmorx-cli/tests/afni_replica_golden.tsv`; Test `cargo test`; Runs in CI: job `test`,
  `cargo test --workspace --locked` (debug); Recorded by
  `LARMORX_GOLDEN_RECORD=1 cargo test --locked -p larmorx-cli --test afni_replica_golden`; artifact
  `golden-mismatches-<platform>` (its `afni-replica/` folder).
- Add: "The AFNI replicas' golden cases moved from `crates-gpl/larmorx-gpl-cli/tests/golden.rs` on
  2026-10-29 with the replicas (into the main package); the checksums did not change (recorded
  from 8218894 on 2026-10-29; they pass through the in-process replica)."
- The coverage rows "`larmorx-gpl afni 3dTshift`" and "`larmorx-gpl afni 3dAutomask`, `3dUnifize`"
  become "`larmorx afni 3dTshift` (the replica)" and "`larmorx afni 3dAutomask`, `3dUnifize` (the
  replicas)"; "For the replicas, the families and tools printed by `larmorx-gpl --help` and
  `larmorx-nc --help`" becomes "For the AFNI replicas, the AFNI tools of `larmorx --help`; for the
  FSL replicas, those of `larmorx-nc --help`"; "the replica has its own in
  `crates-gpl/larmorx-gpl-cli/tests/golden.rs`" becomes "the replicas have their own in
  `crates/larmorx-cli/tests/afni_replica_golden.rs`".

## 12. validation/tools.toml (then `python -m larmorx_validation tables`)

- The three AFNI entries' `how`: replace "clean-room original; exact replica in `larmorx-gpl`" with
  "clean-room original and exact replica, both in larmorx (the replica with CC BY 4.0's attribution)".
- `afni-tshift-replica`'s `larmorx`: "`larmorx afni 3dTshift` with `LARMORX_IMPLEMENTATION=replica` (the replica, `crates/larmorx-afni-replica`, in-process)".
- `afni-automask-replica`'s `larmorx`: "`larmorx afni 3dAutomask` and `lx.afni.automask` with the replica (`crates/larmorx-afni-replica`, in-process)".
- `afni-unifize-replica`'s `larmorx`: "`larmorx afni 3dUnifize` and `lx.afni.unifize` with the replica (`crates/larmorx-afni-replica`, in-process)".

## 13. The site

- content/roadmap.md: row 1 ("The AFNI replicas in the main package", Next) moves to "Completed"
  with "| 2026-10-29 | **The AFNI replicas in the main package.** AFNI 26.3.05 releases the Medical
  College of Wisconsin's code under CC BY 4.0, so the exact replicas of 3dTshift, 3dAutomask and
  3dUnifize left the separate GPL package for LARMORX itself (`pip install larmorx`, Apache-2.0
  with CC BY 4.0's attribution) and run in its process; the C library's sine and cosine they need
  come from Arm's MIT routines, proved equal to glibc's on every input; their records, re-made
  in-process, agree case for case. |"; row 2 (BET's clean-room original) becomes "Next"; "Last
  updated: 2026-10-30".
- content/validation.md: edited in the branch (the "Three ways of reimplementing" paragraph), as
  the brief asked; data/status.yml is generated (`tables --site`, unchanged here: no record's
  numbers changed).
- static/evidence/summary.json: re-export it at the merge (`evidence summary`); the labels
  "larmorx-gpl replica" read "larmorx replica" (`validation/src/larmorx_validation/evidence/summary.py`).

## 14. CI minutes

The six `gpl` jobs' `crates-gpl` steps took 4.0 min per run (three newest runs on main: 4.1, 4.0,
4.0 of 16.1, 14.7, 15.9 min for the six jobs; `validation/investigations/afni-move/ci_minutes.py`).
They are gone. The replicas' debug build and tests join the `test` jobs' `cargo test` (estimated
0.2 to 0.6 min per platform, a part of that back), so the net saving is about 1 to 3 min per run.
