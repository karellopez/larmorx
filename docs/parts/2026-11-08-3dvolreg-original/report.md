## 3dvolreg's clean-room original part: final report (relays 1 to 6 and two spec-writer steps, 2026-11-08)

The last relay's report as returned to the orchestrator, condensed; the full texts are in `notes.md` and `handoff.md`
beside this file. The role: the clean-room implementer (`specs/3dvolreg.md` and the other AFNI specs, AFNI's
documentation and papers, black-box runs of AFNI 26.3.05's `3dvolreg` through the oracle cache; the spec writer's
model run, not read). Branch of 43 commits on 2b453c1, last e3fc0c7c. The part's agents: relays a543551b26b82e03a,
a99eecbb30997199b, a938e0ae66722c27b, a406152e7bc33e465, a7d6c92c90c4edce3, ad8d9af0a170fa8c4; the spec writers
(not implementers; they read AFNI's source) ae4835153236dfbf2 (step 2, its commit cherry-picked as 103bbba) and
ab0e98329b5a93c91 (step 3, 4402541a and c6422e16 fast-forwarded in); their notes and step 3's brief beside this file.

### The original
`crates/larmorx-afni`'s `volreg` module and `cli::volreg`; `larmorx afni 3dvolreg` (`original: true`),
`lx.afni.volreg(implementation="original")` and `"auto"`. Every option, message, exit status and failure case of the
spec; AFNI-86 to AFNI-99 as the replica: the same defaults, `reproduce` and refusals. Validation-only switch
`LARMORX_VOLREG_TIES` (the shear decomposition's exact ties: a fixed side, or `random:<seed>` drawn at every
resampling). The long-name report from the replica part: `output::hidden_name` lets the four AFNI clean-room
originals write output names of 200 to 255 bytes as AFNI does (24 of 24 black-box cases).

- Record `afni-volreg-original` (standard tier, from clean commit 88ecca2, f90bf8f3): 251 cases, 185 pass, 63 refused
  by both alike, 3 expected (AFNI's `.HEAD/.BRIK` output twice, AFNI-96), 0 fail. Exact: exit statuses, messages,
  files, headers, threads (1, 2, 5, 14); `-SAVEDISP`'s products bit for bit on AFNI's raw displacements; the rest
  within §14.3's limits (largest share used: the matrices' rotations 0.947, the registered volumes 0.681).
- fMRIPrep's 10 calls (eight RobustAverage calls on six oracle runs, ds000256's two data-panel runs among them, and
  sdcflows' two PEPOLAR averages) pass; their medians reach at most 0.115 of their limits.
- Speed (shared machine, load 23 to 26): one thread 2.8 to 5.2 times AFNI's speed and 1.5 to 1.9 times the replica's,
  14 threads 9 to 15 times AFNI's; ds005454 AFNI 118 s, replica 73 s, original 42 s (1 thread) and 10.5 s (14); a
  quieter run 75.6 / 40.5 / 23.8 / 4.7 s. Memory within the rule at one thread everywhere (ds005454 332 MiB against an
  allowance of 350); about 1 MiB per added thread.

### The limits (spec §14.3, the spec writer's step 3)
The spec does not describe AFNI's own evaluation of the shear decomposition (§16, the orchestrator's decision of
2026-11-07, on the replica part's spec writer's recommendation). The first record (from 4abf679: 181 pass, 63 both, 6
expected, 1 fail: fMRIPrep's median on ds000256's high-motion run) and the first trial led to a second spec-writer
step, which measured AFNI's own spread under the ties (AFNI takes the sides resampling by resampling: 190 and 144 of
334 ties on that run) and wrote: each registered volume within `max(2e-3, 1e-3 θ_v)` of its largest value (θ_v its
total rotation in degrees) with an NCC limit from it; fMRIPrep's median within the limit its volumes imply;
`-final NN` by a nearest-neighbour rule (within two voxels of AFNI's source voxel); §10.3's trend-line coefficient
order (AFNI's 19 `-SAVEDISP` datasets bit for bit); the seeded spread (12 `random:<seed>` runs, each quantity within
the larger of its limit and 3 times the spread) for random testing; runaways (beyond 15° or 20 mm, or AFNI-97) held
to the output's structure. A re-examination with one tie side for a whole run was not accepted.

### Golden, random testing, defects
Golden: six cases (`cli.afni.3dvolreg` and its `.fmriprep`, `.options`, `.reproduce`, `.SAVEDISP`; `py.afni.volreg`),
86 outputs, recorded from d6151e1 after its smoke tier passed (2,853), verified unchanged from 88ecca2 (246 cases).
Random testing: a check run of 1,813 cases and a half-hour trial of 21,710 (11,385 agree, 10,325 differ); a seeded
sample of 1,200 re-triaged with the final build: 67 agree, 860 within the seeded spread, 268 runaways of the required
structure, 5 explained (3 AFNI-99 named at a two-decimal rounding boundary, 2 at the 12-run spread's edge and within
it at 36 runs); 9 entries in `triage.toml`. No new defect (AFNI-100 to AFNI-110 unused).

### The site
3dvolreg's page: the original's panels on the three ds000005 examples, a new example on ds000256's high-motion run,
the agreement chart (251 cases), the largest share of each limit, fMRIPrep's calls, random testing, speed and memory.
Build 0 problems, numbers 683/683, 32 viewers, inventory after >= before (only `tools/afni-volreg` changed, every count
up), screenshots at 1440 and 390 in both themes. `evidence-blobs` a4fa3b63 (1,236 blobs; 13 added, none replaced).

### Open questions for a spec writer
AFNI-99's naming at a printed two-decimal boundary; 3 times a 12-run spread as a sampled bound (2 of 1,200 at its
edge); the notices' order (printed phase by phase, as the replica). In next-steps.

### Clean-room statements
None opened restricted content (every relay's reads in `notes.md`): relay 1 a grep of `defects.rs` printing the
AFNI-86..99 one-line descriptions (two name AFNI routines); relays 3 to 5 their own `triage.toml` block, relay 4's
diff hunk header one fragment of another tool's (GLIBC-07) entry; relay 3 the spec model's scripts with `--help`
(run, not read); relay 5 a draft import line of another tool's evidence module, never run; relay 6 its own transcript
metadata and the field maps relay's description line.

### Merge notes (the orchestrator, 2026-11-08)
- Merged into main d742dc58 without conflicts. The site's `volreg-original-pages` (ab94b11c) into the site's main
  e4b3757 (b9d118cc); `evidence-blobs` 1b956fc → a4fa3b63 (the manifest's SHA-256 f8a73e3c4db1bb65 matches; no blob
  replaced, 13 added).
- HANDOFF.toml's 55 operations applied (10 files); `register_counts.py` 421 entries (no gap marked "original" left;
  the counts line's date); `tables --site`, `tables --check`, the summary re-exported; the site build 0 problems. Two
  stale texts corrected: the site's planned FSL line (PRELUDE's clean-room original done; FNIRT and its family after
  FreeSurfer) and the register's D13 paragraph ("PRELUDE's clean-room original is the next part").
- The clean-room audit's merge step: 8 agents archived. It first stopped on relay 3's commit 17a186cc, which no agent's
  record made: the command carrying its subject started 30 s before the commit (a wheel build and the formatters
  first, in the background) and the attribution's window opened 15 s before. The window for a command carrying the
  commit's subject now opens 30 minutes before (as for a printed hash), with a test, and the summary no longer stops
  on an unattributed commit. Re-run: 0 blocking; six items judged in `judgements.toml` (the part's own scratch and
  triage entries, the handoff relay's own metadata, one register heading); 3dvolreg's verdict "clean".
- Verified (`verify-merge.sh` on 5e66d1e2, 10:10-10:43, 8 threads): the three Rust workspaces' gates, the GPL and nc
  builds, the Python gates, the smoke tier and golden (every checksum unchanged, only the header); re-runs against the
  committed records, no status or detail differs: `afni-volreg-original` 251/251, `afni-volreg-replica` 190/190,
  `afni-tshift` 317/317, `afni-automask` 130/130, `afni-unifize` 129/129 (the shared output code changed); 3dvolreg's
  page and the summary re-exported unchanged.
