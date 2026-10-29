You are bringing larmorx's antsAI and antsAffineInitializer to ANTs' memory and close to its speed, without changing a single bit of their results (github.com/karellopez/larmorx; your worktree is a checkout of main). This is the small part named in `docs/next-steps.md` ("Memory, the rest" and "antsAffineInitializer's speed") and in the decision log's entry "2026-10-27: Memory similar to the original's". Aim for 3 to 5 hours.

**Testing depth (the user, 2026-10-28):**
- **This part:** decent, smart testing. Prove bit-identity with the standard tier and the smoke tier.
- **Not in this part:** do not re-run the panel or full tiers, and do not build site evidence.
- **Why:** the very extensive testing (the data panel for every tool, full tiers, heavy and nightly random tests, interactive evidence pages) comes in one final validation phase, once all tools are ported.

Read first:
- the repo's `CLAUDE.md`: rules 1-10, "Agent working rules", "Working on this machine";
- `docs/porting-guide.md`: the source as the specification, D11/D13, determinism, the per-tool checklist;
- the decision log's entries of 2026-10-26 and 2026-10-27 on memory;
- `docs/findings/ants-ai.md`: all of it, "Memory and speed against ANTs" above all;
- `docs/validation/ants-ai.md` and `ants-affine-initializer.md`;
- `tests/parity/test_memory_peak.py`;
- `docs/validation/oracle-cache.md`;
- `scripts/orchestration/README.md`;
- one report in `docs/benchmarks/`, as the model.

**Your role: replica author.** You may read the ANTs v2.6.5 and ITK v5.4.5 source in the workspace's `tags/`: both are permissive, and the port lives in the main package. You never edit a clean-room crate (`crates/larmorx-afni`, `crates/larmorx-mri`).

## Task
1. **Memory.**
   - **The main cause:** the metric of antsAI and antsAffineInitializer holds the fixed and moving images in double (`Image<f64>`), where ANTs holds them in float. The values are float already (`pixel_f32` rounds every value), and both programs compute the metric in double. So store the images in float: the storage changes, not the arithmetic.
   - **The rest:** find whatever else keeps larmorx above ANTs. The findings name the allocator's retained pages and the interpreter.
   - **The goal:** `peak <= 1.10 * ANTs + 8 MiB`, on two inputs:
     - `test_memory_peak.py`'s head (128 x 128 x 96);
     - the data panel's 7 T MP2RAGE, where the gap was about 170 MiB.
   - **How to measure:** with `validation/investigations/difftest/memory_pair.py`, against ANTs with CORE-MATH preloaded.
   - **Then:** remove the two `xfail(strict=True)` marks (`_DOUBLE_IMAGES`) once both cases pass.
2. **Speed, at one thread and at several.** antsAffineInitializer takes 277 s against ANTs' 73 s at one thread on the head, and 1212 s against 332 s on the 7 T.
   - **First,** confirm that ANTs really stays on one thread: `/usr/bin/time -v` (CPU %, the counts of voluntary and involuntary switches), and ITK's and ANTs' own thread settings.
   - **Then profile** larmorx (e.g. `perf`) and measure what each part costs.
   - **The D13 notice of ITK-21** is part of the cost. larmorx runs every start point's local search twice, to say where ITK's carried-over learning rate changes the result. Make that check cheap and keep the notice exactly as it is now. For example:
     - run the second search only while the two can still differ;
     - stop it at the first iteration where they agree to the bit;
     - or share the work up to where they part.
   - **Then the metric itself:** the Mattes metric's sampling, interpolation and histogram, as ANTs computes them. Do not reorder any floating-point operation that changes a result.
   - **Target:** at most ANTs' time at one thread on both inputs, and faster with threads (deterministic, independent of `n_threads`). If a gap remains, explain each remaining part with numbers.
   - **Also** check antsAI's `Mattes`/`Regular`/`Rigid` call from the memory test.
3. **Bit-identity kept, proven.**
   - Re-make the standard records of `ants-ai` and `ants-affine-initializer` from clean commits. The oracle cache serves ANTs' runs.
   - Run `scripts/orchestration/compare_records.py`. No status or detail may change against the committed records; any change is a mistake to fix, never to explain away.
   - The golden checksums must not change (`pytest tests/golden`), and the smoke tier (`pytest tests/parity`) must pass.
4. **A short benchmark report,** `docs/benchmarks/ants-ai.md`:
   - **Programs:** ANTs with CORE-MATH (as the records compare) and larmorx at 1 and 8 threads, for antsAffineInitializer and antsAI.
   - **Inputs:** the memory test's head and the 7 T MP2RAGE.
   - **Numbers:** time, CPU time and peak memory, each the median of 3 runs, with the machine's load noted. The machine is shared, so the timings are informal.
   - **Script:** commit it beside the existing benchmarks.
5. **Documents.**
   - **Your own files:** `docs/findings/ants-ai.md` ("Memory and speed": the new numbers, what changed and why each step kept the bits), the two records, `docs/api/ants-ai.md` and `docs/api/ants-affine-initializer.md` where they mention memory or speed, and `test_memory_peak.py`.
   - **No site work.** The antsAI page's "Speed" section says "No benchmark report yet". Put one or two sentences for it in the handoff, with the link to the benchmark report; the orchestrator puts them in.

## Machine (shared: read this)
This machine has 32 threads and 124 GiB. Two other jobs share it:
- **Heavy random-testing cases, run one at a time** (tmux `heavy`, from the sibling checkout `../larmorx-heavy`, in a 60 GiB box). Never touch that checkout, `oracles/difftest/2026-10-25/` or that job.
- **The main checkout** (`/home/karelo/VScode_projects/super_fmriprep/larmorx`). Never touch it.

Your limits:
- **Threads:** up to **24** (`cargo -j 24`, parity `--jobs` to match, `n_threads` <= 24).
- **Memory:** your boxes together at most **36 GiB** (`scripts/orchestration/boxed.sh <GiB> ...`). Every parity tier, benchmark and memory measurement runs in a box.
- **Waiting:** wait for long runs with one background wait on the log's final line, then end your turn until it notifies you.

## Shared documents and IDs
- **Your own files:** those above.
- **Shared documents, through `<scratch>/HANDOFF.md` with the exact text:**
  - the register's rows and entries, and `docs/api/upstream-defects.md`;
  - README and the validation index (generated: edit `validation/tools.toml` and run `python -m larmorx_validation tables`);
  - golden.md, next-steps (the two items marked done), the decision log and CLAUDE.md;
  - the site's antsAI "Speed" sentences.
- **New defects:** only ANTS-190 to ANTS-194 and ITK-116 to ITK-120, each decided by rules A-D and named in the report. None is expected.

## Git and where to work
- **Your worktree branch.** Commit after every meaningful step. Never `Co-Authored-By` or any AI attribution: a commit-msg hook refuses it, and never use `--no-verify`. Do not push or merge.
- **Scratch:** `/tmp/claude-1000/-home-karelo-VScode-projects-super-fmriprep/b322ad6c-679d-4079-be53-839676da5898/scratchpad/antsai-small/`, the folder that holds this brief.
  - Keep NOTES.md there current at every step; HANDOFF.md, builds and outputs go there too.
  - Never write under any `.claude/` folder.
  - Commit every helper behind a result under `validation/investigations/<topic>/`.
- **Environment:** `source scripts/orchestration/env.sh`.
  - Never `maturin develop`: build the wheel into `<scratch>/wheel` and copy the `.so` (`scripts/orchestration/lxv.sh`).
  - Build-cache seed: copy the main checkout's `target/`, `crates-gpl/target/` and `crates-nc/target/` into your worktree, reading only from there.
- **Gates:**
  - `scripts/orchestration/gates-rust.sh <worktree> all` and `gates-python.sh <worktree> --no-build`;
  - `pytest tests/parity` (the smoke tier now passes in full on this machine);
  - `python -m larmorx_validation tables --check`;
  - records and golden checksums from clean commits only.
- **Never:** delete work products; modify the oracle builds, the fMRIPrep runs, the cache's existing entries or the upstream sources; ask for sudo; put FSL on PATH.
- **Public repository:** no network addresses, account names, hostnames or keys in anything you commit.

## Stop after this part; report (concise)
- the memory before and after, per call and input, against ANTs, and each change with why the bits stayed the same;
- the speed: ANTs' thread check, the profile, each change and its gain, and the final numbers at 1 and 8 threads;
- the records re-made and compare_records' result; golden; the smoke tier;
- the benchmark report;
- gates, commits, `<scratch>/HANDOFF.md`, and anything the rules could not settle.
