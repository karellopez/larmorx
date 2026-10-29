# The version update, part 2: FSL 6.0.7.17 → 6.0.7.23 (2026-10-29)

The tools behind `docs/findings/fsl-mcflirt.md` and `docs/findings/fsl-bet.md`, "FSL 6.0.7.23",
and behind the replicas' records against 6.0.7.23. FSL is never on the `PATH`: every script calls
`~/fsl/bin/...` (6.0.7.17) or `~/fsl-6.0.7.23/bin/...` by full path, in a clean environment.

| Tool | What it shows | Run |
|---|---|---|
| `source_diff.py` | every file of the packages the replicas translate, added, removed or changed between the two installations (whitespace and comment-only changes marked) | `python -P source_diff.py` |
| `compare_recordings.py` | mcflirt's 137 recorded runs, 6.0.7.17 against 6.0.7.23 (`scripts/run_mcflirt_oracle.py --fsl-version 6.0.7.23` made the second): 127 identical, 10 changed | `python -P compare_recordings.py <workspace>` |
| `final_interp_probe.sh`, `compare_final_interp.py` | FSL-59: mcflirt 6.0.7.23's `-sinc_final` and `-nn_final` outputs equal its default's | `final_interp_probe.sh <series> <dir>`, then `compare_final_interp.py <dir>` |
| `path_suffix_probe.sh` | FSL-60: names holding `.img` opened under another name by 6.0.7.23 | `path_suffix_probe.sh <series> <dir>` |
| `openblas_core_probe.sh` | mcflirt 6.0.7.23's outputs under every OpenBLAS core this CPU runs: identical (Armadillo 14.2.1's LAPACK inverses do not depend on it here) | `openblas_core_probe.sh <dir> -in <series> -mats -plots -rmsrel -rmsabs` |
| `bet2_compare.sh` | bet2 of both versions and the replica in both modes on one image (the constant image: FSL-41 fixed, then FSL-43) | `bet2_compare.sh <dir> <image> [options]` |
| `step_details.py` | FSL's own step of chosen cases, with each check's detail | `python -P step_details.py fsl-bet <case regex>` |
| `golden_changes.py` | the replicas' golden checksums before and after (3 cases added, 28 BET headers changed: the build string) | `python -P golden_changes.py <old tsv> <new tsv>` |
| `triage_trial.py` | the random-testing trial against 6.0.7.23, sorted by verdict and known reason | `python -P triage_trial.py <night dir>` |
| `memory_fsl.sh` | the BET replicas' memory peaks against FSL 6.0.7.23 (5 of 5 within the rule) | `memory_fsl.sh <log>` |
| `boxed_lxv.sh`, `pytest_parity.sh` | `python -m larmorx_validation ...` and `pytest tests/parity` of this checkout in a memory box | `boxed_lxv.sh <GiB> <log> <args>`, `pytest_parity.sh <log>` |
| `mri-hmc-6.0.7.23.md` | the clean-room head-motion correction's standard tier against mcflirt 6.0.7.23: 2 failures (the known `real/ds003345/default` and FSL-59's `syn/motion/small/nn_final`), so its record stays on 6.0.7.17 | – |
| `bet_triage_run.sh` | one BET call by FSL 6.0.7.23, FSL 6.0.7.17 and the replica (`reproduce`), messages and every output compared (compressed files by content): the trial's BET differences | `bet_triage_run.sh <dir with inputs> <FSLOUTPUTTYPE> <tool> <args>` |
| `bet_repeat.sh` | one FSL BET call run several times: whether FSL's result repeats (FSL-43's memory reads) | `bet_repeat.sh <FSLDIR> <n> <dir> <FSLOUTPUTTYPE> <tool> <args>` |
| `bet2_versions.sh` | bet2 of both versions with several option sets: case 80's difference comes with `-c` on an image stored neurologically (Armadillo 14.2.1's inverse in `niftivox2newimagevox_mat`) | `bet2_versions.sh <image> <dir> "<options>"...` |
| `cog_sign_probe.py` | `fslstats -C` of both versions on small images: 6.0.7.23 prints `-0.000000` for a centre on voxel 0 of an image stored neurologically (case 141's `dc: stack empty`) | `python -P cog_sign_probe.py <dir>` |
| `replay_bet_findings.sh` | the trial's BET findings replayed against both versions, each in a memory box | `replay_bet_findings.sh <log dir> <finding.json>...` |
| `make_permanent_cases.sh` | the six permanent cases of the triage (`validation/difftest/cases/fsl-bet/`) | `make_permanent_cases.sh <findings/fsl-bet> <141's drawn finding>` |
| `export_site.sh` | the BET and head-motion pages' evidence and the summary exported into a site checkout (FSL 6.0.7.23's figures-cache entries computed on the way: `evidence/fsl_step.py`), then `tables --site` | `export_site.sh <site> <log dir>` |
| `site_shots.sh` | before/after screenshots of the two pages (desktop and phone, light and dark) and the number check (`LX_EVIDENCE.check()`), the Validation page after | `site_shots.sh <site before> <site after> <out dir>` |
