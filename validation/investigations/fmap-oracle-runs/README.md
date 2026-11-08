# The field maps' part B oracle runs

The field maps' part B (sdcflows' PEPOLAR, SyN and direct field maps, niworkflows' helpers and
fMRIPrep's own field-map steps; 2026-11-08) needs fMRIPrep 25.2.6 runs on the paths no earlier run
reached (the part's decision D8). They are made by the orchestrator, boxed, as the earlier runs
(`scripts/run_fmriprep_oracle.sh`, `--fs-no-reconall`, 4 threads), each with its BIDS view recorded
in its `RUN_INFO.txt`.

- **SyN** (`--use-syn-sdc`) on the data panel's views of ds000228 (sub-pixar008), ds004884
  (sub-M2079) and ds005003 (sub-RS001): runs `<dataset>-sub-<label>-noreconall-syn`.
- **PEPOLAR and direct field maps** on the views `derived_views.py` writes (its docstring says
  what each changes): ds006736 (two opposite-direction EPIs), ds004958-pepolar (a spin-echo pair
  linked to its BOLD), ds002603-epi (two EPIs with their own BOLD runs), ds005752-direct (an Hz
  field map linked to two BOLD runs).

```bash
python -P derived_views.py <workspace>     # the four views and their <view>.DERIVED.txt
```

Every changed sidecar is a new file (the cache's hard link replaced), its SHA-256 and the
original's recorded in `<view>.DERIVED.txt` beside the view; no image data is changed.
