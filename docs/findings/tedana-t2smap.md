# Findings: tedana's `t2smap` (tedana 26.0.3)

**Restricted: written from tedana's source.** A clean-room implementer must not read this file
(`docs/porting-guide.md` §1); what it learns about behaviour is in `specs/t2smap.md`.

What the replica (`crates-gpl/larmorx-gpl-tedana`, the `larmorx-gpl` program, 2026-11-08, from
tedana 26.0.3) found in `t2smap`: where tedana's code does something questionable, how each case
was decided (D13's rules A to D) and what larmorx does. Each entry says how it is known: *read*
(the source), *history* (tedana's repository, its pull requests and release notes), *measured*
(`validation/investigations/t2smap/`). The defects' register rows are TED-01 to TED-15
(`docs/findings/upstream-defects.md`). The facts of the inventory and of the numerics (numpy's
reductions, `lstsq`, SciPy's `curve_fit`, nibabel's and nilearn's paths) are added with the
part's validation (relay 7).

## TED-05: the full T2* map is no longer capped (rule A)

- **What tedana 26.0.3 does** (*read*). `decay.modify_t2s_s0_maps` applies the floors (infinite
  T2* to 500 ms, T2* <= 0 to 1 ms, the floor where `exp(-TE / T2*)` underflows) to the full map,
  copies it into the limited map with the one-echo voxels set to 0, and caps **only the limited
  map**: values above ten times its 99.5th percentile (`scipy.stats.scoreatpercentile`,
  `interpolation_method="lower"`) are set to that percentile. `t2smap_workflow` writes the full
  map (`T2starmap`, which fMRIPrep keeps as its T2* map) and weights the optimal combination with
  it, uncapped.
- **What tedana 25.0.1 did** (fMRIPrep 25.2.6's tedana; *read*). `t2smap_workflow` capped the
  full map itself after the fit, with the same rule (its comment: "set a hard cap for the T2*
  map/timeseries ... anything that is 10x higher than the 99.5 %ile will be reset to 99.5 %ile"),
  besides `modify_t2s_s0_maps`' cap of the limited map; tedana's main workflow (`tedana.py`) did
  the same.
- **The change** (*history*). Pull request ME-ICA/tedana#1290, "Write out T2* & S0 variance +
  T2*-S0 covariance" (merged 2026-01-08, released in 26.0.0 on 2026-02-25; closes #1268, "Track
  and write out S0 and T2* estimate variance (and maybe covariance?) maps"), removed the
  workflow's cap of the full map (`tedana/workflows/t2smap.py`: the four lines of `cap_t2s`) while
  restructuring the fit: its description lists "Create new function to create the limited maps
  from the full maps and apply the ceilings and floors", that is, the floors and caps were meant
  to move into `modify_t2s_s0_maps`, which caps the limited map only. Neither the description, its
  seven comments and review, nor the 26.0.0 release notes (which list the pull request only by its
  title) mention the full map's cap or a reason to remove it; tedana's documentation
  (`docs/outputs.rst`) describes `T2starmap` as the "Full estimated T2* 3D map" without one. The
  current main branch (checked 2026-11-08) still caps only the limited map.
- **Measured** (`validation/investigations/t2smap/defects/ted05_full_map_cap.py`, relay 5b): on
  fMRIPrep's four multi-echo t2smap calls (ds003592 sub-280, ds000210 sub-02 cuedSGT and rest,
  ds000258 sub-21262), 2 to 34 voxels per call hold full-map values above ten times the cap, up
  to 239,794,244 ms (239,794 s: not a T2*), where tedana 25.0.1's t2smap, fMRIPrep's own, wrote
  the capped value.
- **Decision: rule A** (the input makes sense and the result is wrong), not rule C: the removal
  came with a restructuring whose description says the new function applies "the ceilings and
  floors", with no word of dropping the full map's; it is not a design pipelines rely on (fMRIPrep's own tedana capped the full map). By default the full map gets the
  limited map's cap: values above ten times the limited map's 99.5th percentile are set to it
  (tedana 25.0.1 took the full map's own percentile, which differs from it only by the one-echo
  voxels the limited map holds as 0); the notice names how many values were capped, and the
  threshold. `LARMORX_UPSTREAM_DEFECTS=reproduce` (`upstream_defects="reproduce"`) writes
  tedana 26.0.3's uncapped map and weights.
