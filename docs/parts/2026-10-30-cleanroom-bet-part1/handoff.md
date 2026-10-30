# HANDOFF: BET's clean-room original, part 1 (2026-10-30)

Branch: the worktree agent-a8aa142043f423b7f (from bed7e04). The exact text for each shared
document; numbers are those of the record `docs/validation/fsl-bet.md` (commit named there).

## validation/tools.toml

In the `[[tool]]` `fsl-bet`, replace `how` and `validation`:

```toml
how = "clean-room original (`bet2` and the `bet` script; betsurf to come), `experimental`; exact replica in `larmorx-nc` (FSL Licence, non-commercial)"
validation = "original: [{fsl-bet.pass} of {fsl-bet.compared}](record:fsl-bet) compared cases within thresholds against FSL 6.0.7.23, fMRIPrep's {fsl-bet.fmriprep} BET calls on every oracle run with BOLD runs bit-identical (masks, brains, outlines and meshes FSL's wherever the first pass does not self-intersect; the reruns open, a question for the spec writer); [2.6 to 3 times faster than FSL on one thread, 6 to 11 times on 24](repo:docs/benchmarks/fsl-bet.md); replica: [{fsl-bet-replica.pass} of {fsl-bet-replica.compared}](record:fsl-bet-replica) compared cases bit-identical to FSL 6.0.7.23, every file, exit status and message ([{fsl-bet-replica-full.pass} of {fsl-bet-replica-full.compared}](record:fsl-bet-replica-full) in the full tier against FSL 6.0.7.17, the data panel's T1w images included), fMRIPrep's {fsl-bet-replica.fmriprep} BET calls on every oracle run with BOLD runs included; {fsl-bet-replica.both} more rejected by both, alike; {fsl-bet-replica.filled} where FSL crashes and `reproduce` fills what it leaves undefined; {fsl-bet-replica.memory} where FSL's fill reads memory beyond the volume (FSL-43, undefined); `bet -S`, `-B`, `-A`, `-A2` pending (FLIRT, FAST)"
```

A new `[[record]]` before `fsl-bet-replica`:

```toml
[[record]]
id = "fsl-bet"
larmorx = "`lx.fsl.bet(implementation=\"original\")`, `larmorx fsl bet2`, `larmorx fsl bet` (clean-room, `crates/larmorx-fsl`)"
reference = "bet and bet2 (FSL 6.0.7.23)"
status = "`experimental` ({fsl-bet.fail} of {fsl-bet.compared} compared cases miss their threshold: the surfaces whose first pass self-intersects; fMRIPrep's {fsl-bet.fmriprep} calls included; {fsl-bet.both} rejected by both, with the same messages and status; {fsl-bet.filled} where FSL crashes and `reproduce` fills what it leaves undefined; {fsl-bet.pending} pending FLIRT and FAST; betsurf to come)"
```

(`validation/src/larmorx_validation/tables.py` already maps `fsl-bet` to `_bet` in the branch.)
Then `python -m larmorx_validation tables`.

## README.md

Generated from tools.toml (above). Nothing by hand.

## docs/next-steps.md (item 7, BET)

Replace the BET clean-room line with:

> **BET's clean-room original: part 1 done (2026-10-30).** `crates/larmorx-fsl`: `bet2` complete
> (every option, message and failure case of specs/bet.md §4, §7, §9: FSL-40, -42, -43, -56, -60,
> -61, -62 with `reproduce`), the `bet` script's default, `-R` (FSL-63), `-F`, `-Z` (FSL-45),
> FSL-44, FSL-46, the pending `-S`, `-B`, `-A`, `-A2`; `lx.fsl.bet(implementation="original")`,
> `larmorx fsl bet2|bet` (`auto`: the replica when installed, else the original). Record
> [fsl-bet](validation/fsl-bet.md): 108 of 111 compared cases within the thresholds (fMRIPrep's
> 29 calls bit-identical; 202 of 217 brains, masks and outlines bit-identical; the 3 misses: real
> T1w images whose first pass reruns); built for speed (benchmark [fsl-bet](benchmarks/fsl-bet.md):
> 2.6 to 3 times FSL's speed on one thread, 6 to 11 times on 24); memory within the rule (`tests/parity/test_memory_peak.py`: bet2 and `bet -F` below FSL's, `bet -R` 117 MiB against 103 plus the interpreter's 13). **Part 2:** betsurf's original (spec §6), the site page's
> clean-room panels, and — after the spec writer answers — the reruns and the skull search.
> **Spec questions** (for the spec writer, from [findings/fsl-bet-original.md](findings/fsl-bet-original.md)):
> (1) the rerun's smoothness rule (§4.5: FSL's first rerun differs from the rule as written though
> its factor 100 and its 1000 iterations are right; the three standard-tier cases that rerun miss
> the thresholds, Dice 0.980 to 0.995); (2) the skull search's details (§4.7: Dice about 0.9);
> (3) the quaternion's precision path (§8: the last float bit of one component in some real
> headers, about 5e-9 in NIfTI-2's doubles; the record allows 2^-23); (4) `-e`'s exact test
> (§4.5: FSL finds intersections where a segment-triangle test, with folds and coplanar overlap,
> does not, on small random heads: difftest 2026-10-30 #27); (5) the order of the sums over a
> vertex's neighbours (§8 gives the triangles' only: it decides the last digits on degenerate
> surfaces); (6) move the black-box observations of the findings into the spec, marked
> (observed): the mesh numbering, the rasterisation, the robust range's rounding (the width in
> float), small images (`n/50 = 0`), the fill from any start, the option parser's values, ANALYZE
> inputs, single slices written 2D, `--ct`'s outline in Cormack units, FSL-56's layout, FSL-62's
> brain, `bet`'s variation numbers, its failure line, FSL-44's splitting, `-F`'s and `-Z`'s files.

## docs/validation/golden.md

Add to the list of cases (FSL section): `cli.fsl.bet2` (every output of bet2, verbose, an
oblique image stored with a positive determinant), `cli.fsl.bet2.options` (the numeric options,
`-t`, the search depths, `--ct` with `--binaryMask`), `cli.fsl.bet2.reproduce` (FSL-43's fill by
the linear index on a single slice, FSL-40's names, and the default on the same calls),
`cli.fsl.bet` (the script, verbose), `cli.fsl.bet.variations` (`-R` both modes (FSL-63), `-F`,
`-Z` both modes (FSL-45)), `py.fsl.bet` (the Python wrapper with the original, the robust
variation). `lx.fsl.bet` is no longer replica-only (`REPLICA_ONLY_FUNCTIONS` keeps betsurf).

## docs/findings/upstream-defects.md and docs/api/upstream-defects.md (summary rows, behaviour only)

- FSL-62's row: "... writes the mask (as read: its 0 and 1), masked, as the brain image"
  (observed by the original's implementer: the mask is not the converted one).
- FSL-56's row: "... FSL writes the extensions in full and the data right after them; only the
  recorded `vox_offset` is wrong" (observed).
No new defect (FSL-67, FSL-68 unused).

## The random-testing trial (for the night's triage)

`<workspace>/oracles/difftest/2026-10-30-cleanroom-bet` (30 minutes, 718 cases, seed 20261030,
both implementations): the replica agreed in every case it finished; the original 334 of 573
(betsurf's 143 skipped, 2 timeouts of both). Its 239 differences were run again with the final
build (`validation/investigations/cleanroom-bet/retrial.py`): 164 agree; the 75 left, by failed
check: the messages (29: the self-intersection total's last digits on small heads, one `thresh`
digit), meshes (15: sub-voxel, small heads), brains and masks (25: small heads, reruns), outlines
(5), a header (1). No defect of FSL among them (FSL-67, FSL-68 unused). They are the clean-room
original's, reported with their reproducers (`difftest --case fsl-bet:20261030:<n>`), for
triage.toml by the orchestrator (the implementer does not open it).

## The site (larmorx_documentation): Roadmap

Row "BET clean-room original": status "part 1 done (2026-10-30): bet2 and bet's -R, -F, -Z;
betsurf and the site's panels in part 2". No other site work in part 1.

## Spec questions (for the spec writer)

The six of next-steps above, with the evidence in `docs/findings/fsl-bet-original.md` and the
probes in `validation/investigations/cleanroom-bet/`.
