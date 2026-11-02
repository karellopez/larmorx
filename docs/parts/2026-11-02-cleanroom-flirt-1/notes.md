# Clean-room FLIRT part 1: notes

Worktree: <workspace>/larmorx/.claude/worktrees/agent-a083655161a925b75 (branch worktree-agent-a083655161a925b75, from main 3f07df7)
Helper scripts (scratch bin/): w.sh (env), wheel.sh (maturin build + copy _core), case.sh <regex> [jobs]
(the fsl-flirt suite against target/release/larmorx), probe.sh <name> --case X | --args "..." (keeps
dirs in scratch p/<name>), costs.sh (costs.sch: costs of two matrices at each scale, FSL vs original).

## Done
- Shared engine (larmorx-mri hmc): line_search_from (first step guess), Accumulator::point_at +
  Weighted/sample_weighted, correlation() public, *_weighted cost variants, TestBins.fuzzy. hmc passes
  None/10.0/0.5: same code path.
- crates/larmorx-fsl/src/flirt/: mat, pyramid, cost, bbr, optimise, search, schedule (default 3D/2D
  schedules as `flirt -v` prints them), output, xfm, engine; cli/flirt.rs, cli/convert_xfm.rs.
- larmorx-cli: flirt, convert_xfm original:true, routed to larmorx_fsl::cli.
- Suite validation/.../parity/fsl_flirt.py (thin, reuses fsl_flirt_replica cases + oracle cache keys).
- WIP commit 6041701.

## Black-box findings so far (for docs/findings/fsl-flirt-original.md)
- WARNING "Both reference and input images have an sform" only with -v.
- Erroneous dof message: twice per optimise row, once per search minimum, none for measurecost (73 = 36*2+1).
- dualsort unequal: message without newline; setoption needs >= 3 words always (nosubset too); blank line: nothing.
- printparams: each row + empty line; saveparams: + final empty line; save: rows + empty line.
- costs at every scale (costs.sch): normcorr, MI, normmi, labeldiff, leastsq match FSL to 6-7 digits;
  weighted normcorr/leastsq/MI match. corratio: matches at 4/2/1 mm and smoothing 0..16, NOT at the 8 mm
  pair with 32 or 64 bins (16 bins match), nor with -noresample (3.5e-5 rel); woods (FSL: sum n_b sd/mean / n,
  not n_b^2) same pattern; weighted corratio: FSL gives >1 (1.016) at 8 mm: formula unknown -> spec question.
- -noresample still blurs the input at setscale (normcorr matches only so).
- pyramid levels (-debug refvol*): last-bit differences in 2.5% of voxels (blur+isotropic); spec's rule
  followed (variants tried: reversed order, f32 products, f64 sums: all worse).
- sinc output (-interp sinc -applyxfm): last-bit differences (~1e-7 rel) in 1/3 of voxels; variants tried
  (f64 sums, product order, table domain 3.5): none exact -> threshold 1e-5 rel + spec question.
- FSL writes the sform's -0 as +0.

## State (2026-11-02 ~13:30)
- Commits: WIP1 6041701, WIP2 855dc56, WIP3 0231f5b, WIP4 751ef82, WIP5 8847fb9 (with trace hooks,
  removed after). Uncommitted now: hooks removed; minima z-outer order; convergence count = subset
  size (optimise.rs n_tol); suite run_original no LXFLIRT passthrough.
- Found today (findings doc updated): sparse bins (<=2 points) left out of corratio/woods entirely
  (synthetic proof, costs now FSL's at every pair); reset pair keeps centre (centre.sch); search
  re-measures COGs (warnings); -applyisoxfm keeps srow when sform_code 0; search grid uses the
  search command's dof (scale even with -dof 6), sampled params (no decompose), P start costs,
  minima z-outer, subset convergence count; pyramid + input blur bit-identical (-debug).
- Remaining: last-bit cost arithmetic (f32 variants tried, none exact); paths drift 1e-3 at 8 mm.
- Direct FSL runs outside the cache so far: ~54 (probes, cogx 8, verbose-7 trace, -debug, save.sch).
- Trial triage tools: scratch cats.py, show.py, sample.py, bin/replay.sh (8 at a time, 32 GiB box).

## Done (2026-11-02 ~19:20): part 1 complete, reported
- Final commits: b244d75 (code, records from it), 13012f3 (golden registry), 77d8fe1 (golden.json),
  63f8522 (record + API page). Record fsl-flirt: 89/111 compared within thresholds, 35 both-error,
  4 expected, 22 fail, 7 skipped. mri-hmc identical. Smoke tier passed on b244d75 (2653).
- HANDOFF.md complete. Nothing running.
