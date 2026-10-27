# Moving the workspace to the new main machine

The new laptop (16 cores / 32 threads, 128 GB, RTX 4090, Ubuntu 22.04) becomes the main machine;
the current laptop becomes the quiet benchmark machine and a second Linux reference (decision log,
2026-10-25 and 26). This page is the move, step by step: what the user does (steps 1 to 7, a few
commands, some with `sudo`), and what the orchestrator does in its first session there (step 8).
The scripts are in `scripts/orchestration/move/`.

**Same user, same paths** (the user, 2026-10-27): user `karelo`, workspace
`/home/karelo/VScode_projects/super_fmriprep`. Every absolute path in the scripts, records,
worktrees, the oracle-output cache and Claude Code's memory notes then stays valid.

**What moves** (about 160 GiB without build caches; measured 2026-10-27 in `MOVE-MANIFEST.json`):
the workspace (`oracles/` 134 GiB: the fourteen fMRIPrep runs, the oracle builds and toolchains,
the oracle-output cache, the fMRIPrep image; the repositories with their git
state and agent worktrees; the upstream clones and `tags/`; the Python environment `.venv`; the
usage tool `tools/claude-usage/`), FSL 6.0.7.17 as installed (`~/fsl`, the FSL replicas'
oracle), the test data (`~/.cache/larmorx-testdata`), Rust as pinned (`~/.cargo`, `~/.rustup`),
Claude Code's memory notes and transcripts of this project, and the usage tool's records
(`~/.cache/claude-usage/`). The usage tool reads the plan's 5-hour and weekly windows with
Claude Code's own program (`claude -p /usage`, no model call); on the new laptop that is the
program inside the VS Code extension (the tool finds it), working once Claude Code is signed in
(step 7). Build caches (`target/`) are not
copied: the new machine rebuilds them in minutes. Left on the old laptop, to save transfer time (fetched over SSH
if ever needed; `manifest.py` `LEFT_ON_OLD`): `archive/` (55 GB of old session scratch),
`oracles/compare/` (the 2026-10-12 fMRIPrep stage comparisons, 22 GB), the GCC 11 ANTs build
kept as evidence (4.3 GB) and mcflirt's raw band runs (18 GB; the records read only
`band/band.json`, which is copied, and the FSL update re-measures the band). Nothing is deleted on either machine; the old
laptop keeps its copy untouched until the proof (step 8) has passed.

## The user's steps

1. **Prepare the new laptop** (as the old one: Ubuntu 22.04, glibc 2.35):
   ```bash
   sudo apt update && sudo apt install -y rsync git tmux build-essential pkg-config openssh-client
   sudo add-apt-repository -y ppa:deadsnakes/ppa
   sudo apt install -y python3.12 python3.12-venv python3.12-dev   # the venv's base: /usr/bin/python3.12
   ```
   - **Apptainer 1.4.5** (the version the fMRIPrep oracle runs used): the `.deb` from
     github.com/apptainer/apptainer/releases (`apptainer_1.4.5_amd64.deb`), `sudo apt install ./apptainer_1.4.5_amd64.deb`.
   - **VS Code** with the Claude Code extension; sign in to GitHub in VS Code (pushes go through
     it; no tokens or passwords through Claude). Node 22 and Google Chrome (the site's
     screenshots and JavaScript check: the old laptop has node v22.23.2, Chrome 135).
   - Do not start Claude Code in the workspace until step 7.
2. **Let the new laptop reach the old one by SSH key** (once; the user types the old laptop's
   password once, to `ssh-copy-id`, never to Claude):
   - on the old laptop: `sudo apt install -y openssh-server` (it is already running there on
     2026-10-27), and `hostname -I` gives its address (on 2026-10-27: 10.2.167.242);
   - on the new laptop: `ssh-keygen -t ed25519` (Enter for the defaults), then
     `ssh-copy-id karelo@<old address>`, then `ssh karelo@<old address> true` must work without
     a password. A network cable between the two (or the same wired network) makes the copy
     much faster than Wi-Fi (about an hour at 1 Gbit/s, several over Wi-Fi).
3. **On the old laptop, make sure nothing runs** (no agent, no parity run): the copy must see a
   still workspace. The orchestrator ends its session at a clean stop before the move.
4. **The FreeSurfer licence** for the fMRIPrep oracle runs (only mounted into the container,
   never committed; the user copies it):
   ```bash
   scp karelo@<old address>:/usr/local/freesurfer/license.txt /tmp/fs-license.txt
   sudo mkdir -p /usr/local/freesurfer && sudo mv /tmp/fs-license.txt /usr/local/freesurfer/license.txt
   ```
5. **Copy everything** (resumable: if it stops, run the same command again):
   ```bash
   scp karelo@<old address>:/home/karelo/VScode_projects/super_fmriprep/larmorx/scripts/orchestration/move/pull.sh /tmp/
   bash /tmp/pull.sh karelo@<old address>
   ```
   It ends with `done` (or `NOT COMPLETE`: run it again). Its log is `~/move-pull-<date>.log`.
   On 2026-10-27 `second-pass.sh` (tmux session `move2`) waited for it, copied again what
   changed meanwhile, set up the shell (Rust, Node 22) and ran `check.py` (step 6): its summary
   is `~/MOVE-READY.txt`.
   Before it: anything the new laptop already has at the copied places and that differs (on
   2026-10-27 a `.venv` on uv's Python and FSL 6.0.7.10 in `~/fsl`) is moved aside to
   `~/archive-before-move-<date>/`, never merged with the copy (rsync adds and replaces files,
   it does not remove the old ones, so two installations would mix). Over Wi-Fi the copy runs
   at about 4 MB/s (about 11 hours for 160 GiB); with both laptops on a cable, about an hour.
6. **Check the new laptop:**
   ```bash
   python3 /home/karelo/VScode_projects/super_fmriprep/larmorx/scripts/orchestration/move/check.py
   ```
   Every line should be OK. A WARN on `libm` or `glibc` means the originals could compute
   differently there: the proof (step 8) measures it. A FAIL says what to fix.
7. **Start Claude Code** in VS Code in the folder `/home/karelo/VScode_projects/super_fmriprep`.
   Its settings: `~/.claude/settings.json.from-old-laptop` is the old laptop's (the status line,
   agents' caches kept an hour); Claude merges it in step 8. Then say: **"continue with the move:
   docs/machine-move.md, step 8"**.

## Step 8: the orchestrator's first session on the new machine

On 2026-10-27 the old laptop's session did steps 2 to 6 itself over SSH (the user ran only the
`sudo` lines); what it did and found is in the workspace's `ORCHESTRATOR-STATE.md`. The new
session starts here, in this order, whatever the user's first message says:

1. **Where am I.** `hostname` must print `karelo-Alienware-m18-R1-AMD` (the old laptop is
   `karelo-GE75-Raider-8SF`). The session-start checks of CLAUDE.md: `uname -m`, `nproc`, free
   memory, `apptainer --version`, `rustc --version`.
2. **Usage.** `python3 tools/claude-usage/usage.py check` (as before every part). It must print
   GO, SLOW DOWN or PAUSE with the windows read "0 min ago"; "could not read the limits" means
   Claude Code's program was not found or is not signed in. SLOW DOWN or PAUSE: do steps 3 to 5
   (machine time, few tokens) and leave the queued work (8) for later.
3. **Claude Code's settings.** Merge `~/.claude/settings.json.from-old-laptop` into
   `~/.claude/settings.json`: keep the new machine's own keys and add `statusLine` (the command
   path is the same) and `subagentPromptCacheTtl: "1h"`. Both were the user's choice on
   2026-10-27; tell the user in one line, and that they take effect in the next session.
4. **The check, again.** `python3 larmorx/scripts/orchestration/move/check.py`: every FAIL fixed;
   each WARN either fixed or named in the proof (5) as a possible cause.
5. **Build and prove** (machine time; boxes up to about 100 GiB on this machine):
   - **Build:** `source larmorx/scripts/orchestration/env.sh`, then in `larmorx/`:
     `maturin develop --release` (main checkout only), `scripts/orchestration/build-gpl.sh .`,
     `scripts/orchestration/build-nc.sh .`; the gates (`gates-rust.sh . all`, `gates-python.sh .`).
   - **The proof** (next-steps, item M): the results do not depend on the machine.
     - `pytest tests/parity` (the smoke tier) and the golden suite: unchanged checksums.
     - Every record's standard tier re-run (`python -m larmorx_validation parity <suite> --tier
       standard`, boxed), compared case by case with the committed records
       (`scripts/orchestration/compare_records.py`): identical results, timings aside.
     - The originals themselves: a sample of the oracle-output cache re-run and compared
       (`python -m larmorx_validation oracle-cache verify`), so a different libm would show.
     - Any difference is a finding (`docs/findings/`), explained before new work.
     - The new machine is AMD (Ryzen 9 7945HX, AVX-512), the old one Intel (AVX2): runtime SIMD
       dispatch takes other paths here, so the proof covers that too.
6. **The machine's settings in CLAUDE.md** ("Working on this machine", "Machines"): 32 threads and
   124 GiB (one agent may use up to 28 threads, parity `--jobs` to match; boxes together up to
   about 100 GiB); the old laptop as the benchmark machine and second reference, reached by SSH
   (`docs/helper-machines.md`; add the `old-laptop` git remote there).
7. **Push** what the old laptop committed and did not push (named in `ORCHESTRATOR-STATE.md`),
   after the gates and the proof: GitHub sign-in through VS Code (the askpass socket, as before).
8. **The queued work:** the panel's fourteen whole template registrations
   (`parity ants-registration --tier panel --case 'register_template.*/registration$'`, about 67
   hours of CPU, several at once) and antsRegistration's standard record re-made (513 cases since
   random testing's permanent cases); the 25 heavy random-testing cases that waited for a larger
   machine; then the small part for antsAI and antsAffineInitializer; then the data panel's
   part (b) (AFNI, FSL and I/O) (`docs/next-steps.md`). The Windows laptop's SSH when the user
   asks (`docs/helper-machines.md`).

## Afterwards

- The helper machines (the old laptop for speed measurements and as a second Linux reference,
  the Windows laptop, later the lab's server): `docs/helper-machines.md`.
- Work continues only on the new machine; the old laptop's workspace stays as it was (a frozen
  copy) until the proof has passed, then serves the benchmarks.
- `MOVE-MANIFEST.json` (workspace root, not in git) records the old machine's facts and every
  oracle program's SHA-256; `check.py` can be run again at any time.
