# Final report: AFNI-23 fix + explicit Python settings

Agent `a3b361d527b599c1e`, 2026-10-12. The agent's report to the orchestrator, verbatim.

I made both changes; the code, docs and validation records are committed in the worktree branch `worktree-agent-a3b361d527b599c1e`, not pushed. All gates passed before each commit, and both standard-tier 3dTshift suites were re-run from the clean merged commit `c479c1a`.

## AFNI-23: what I chose
- **What the variable is for.** `AFNI_NIFTI_SCALE` is documented nowhere and read in one place in AFNI. I checked AFNI 16.0.00 and 20.3.03, plus AFNI's own change history:
  - Until 2021 it meant "do not scale": uint8/int16 data were read as stored, slope and intercept both ignored.
  - In 2021 AFNI started keeping scaled integers as integers, with the slope as a scale factor. Since then the variable no longer stops the slope; its only remaining effect is to keep integers with an intercept as integers, which is what loses the intercept.
- **The choice: keep both in the header.** The output keeps AFNI's stored integers and slope (slope 1 where AFNI writes none) and gets the input's intercept as `scl_inter`. The variable's remaining purpose, integers stay integers, is kept, and only the header differs from AFNI's output. Applying both scalings everywhere would have made the variable do nothing.
- **Two cases come from `-rlt`**, which removes the trend and the intercept with it:
  - `-rlt` without `-ignore`: AFNI's output is already right, so nothing changes and nothing is named.
  - `-rlt -ignore n`: the ignored volumes need the intercept and the others don't, which one `scl_inter` can't describe. Here the data are read as without the variable (float32).
- **Oracle checks by hand** (int16 and uint8, slope 1, every restore mode, `-no_detrend`, `-tpattern zero`, a copy):
  - `reproduce` equals AFNI's output with the variable, data and header.
  - The default reads back within half a stored unit (at most 0.185, 0.25 and 0.5) of the run without the variable.
  - The `-rlt -ignore` default equals AFNI's output without the variable, byte for byte.

## Parity (311 cases each, from `c479c1a`)

| | Result |
|---|---|
| Replica, `reproduce` | 279 of 279 compared cases bit-identical to AFNI, including the 4 AFNI-23 cases (3 new: copy, `-rlt`, `-rlt -ignore`); all 60 global-option/variable cases identical in exit status and messages too |
| Replica, default | AFNI-23 named in exactly the 4 cases where the output differs, and not in `-rlt` |
| Original | unchanged (223 pass, 163 bit-identical); the 3 new cases are expected divergences until part 2b |

The spec text for the clean-room original is in `specs/3dTshift.md` §7.1, in behaviour terms with the observed numbers. The register entry is now class c, decided, with the gap marked for the original.

## The Python settings API
- **New keyword:** `lx.afni.tshift(..., afni_settings: Mapping[str, str] | None = None)`. The Python API no longer reads the process environment at all.
- **What the replica gets:** exactly these settings, passed as environment variables so values arrive as written. Every AFNI variable in the caller's environment is removed first, whatever its case.
- **Settings the wrapper fixes itself:** the five it needs (a path is one file, plain messages, no oblique report). Passing any of them is a `ValueError`.
- **Validation:** an unknown or wrong-case name, or an empty value, is a `ValueError`. I refused empty values because Windows may not pass them on the same way.
- **The original:** six variables change the result (`AFNI_NIFTI_PRIORITY`, `_SCALE`, `_VIEW`, `AFNI_DEFAULT_STD_SPACE`, `AFNI_NIFTI_WRITE_TYPE`, `AFNI_NOREALPATH`). The original raises `NotImplementedError` naming them, including under `implementation="auto"`; variables that change nothing a call returns are accepted.
- **Command line:** both still read the environment.
- **Result scaling fix:** the result now applies `scl_inter` as `lx.load` does.

**Tests:**
- **Python, all six platforms:** a stand-in replica program records what it receives. Variables set in `os.environ` (including lower-case and `NIML_*` names) never reach it; `afni_settings` does, and nothing else.
- **Python, other tests:** the name and value checks, the original's `NotImplementedError`, and the intercept scaling.
- **Rust:** a GPL CLI test of AFNI-23 in both modes, and 3 GPL golden cases (no existing checksum changed).
- **Parity:** the replica's variable and global-option cases now also call the Python API with `afni_settings`, while the process environment holds AFNI variables that would change the output if they leaked. All 28 cases it can express give the command line's output bit for bit.

## Docs
Updated:
- `docs/api/afni-tshift.md`: a new "AFNI's settings in Python" section, the environment rule for the command line against Python, the AFNI-23 row, and new counts;
- `docs/api/upstream-defects.md`;
- the spec (§7.1, §8, §9);
- the register;
- the restricted findings;
- porting guide §9.1;
- the decision log;
- `golden.md` and the replica's `PROVENANCE.md`.

`README.md` now quotes the current records (311 cases; 223 of 223 agree, 163 bit-identical; replica 279 of 279). No `.pyi` change was needed: the Rust binding didn't change and `tshift.py` is typed inline.

## Commits
- `8997020` replica AFNI-23 and the Python settings API, with tests and provisional golden checksums
- `98fd2e0` spec, register, findings, docs and parity cases
- `c479c1a` merge of `main` (your CLAUDE.md commit)
- `8d9244a` validation records from `c479c1a` and golden checksums recorded from it; README
- `9a5ea24` decision log reworded: AFNI's history in behaviour terms, source details kept in the restricted entry

## For you
- **Shared venv:** `maturin develop` pointed it at this worktree (`larmorx.pth`). I only ran it after your verification run had finished; step 2 of the merge checklist in the main checkout points it back.
- **`LARMORX_GPL_BIN`** is still read by the Python API, to find the replica program. I left it, since it decides where the program is rather than how it computes; say if you want that explicit too.
- **One corner is named inconsistently.** With a negative slope, an intercept, the variable, `-rlt` and `-ignore` together, the default doesn't name AFNI-01 but `reproduce` does. There is no parity case for it.
- **CLAUDE.md** still quotes the old 3dTshift numbers ("217 of 217"); I didn't edit it.
- The parity log is `oracles/logs/tshift-afni23-settings.log`.
