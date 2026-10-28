# HANDOFF: tests that depend on the machine (2026-10-28)

Branch `worktree-agent-a8060dfaf5aa8669b` (from `c223e64`; commits 831c754, e682a7e, efb8536,
224bbac, 051d7ef, c2ce1f9). Validation code and its own documents only: nothing larmorx computes
changed (no Rust; no site changes: no tool's evidence changes). No new defect, no defect ID used.
Integrate the blocks below into the shared documents.

## 1. `docs/next-steps.md`: the item "Tests that depend on the machine, from the move's proof"

Replace the whole bullet (from `- **Tests that depend on the machine, from the move's proof**` to
the end of its third sub-bullet, "... marked never-cached.") with:

```markdown
- **Tests that depend on the machine, from the move's proof**: **done 2026-10-28**
  ([machine-move-proof.md](findings/machine-move-proof.md), "Fixed"). The originals run under a
  fixed address-space limit (64 GiB, `LARMORX_ADDRESS_SPACE_LIMIT`; the largest real call 9.4 GiB,
  the smallest refusal the records expect 96 GiB; random testing derives it from each case's
  memory budget, so `--heavy` runs under twice its box), so `allocation-refused` and `MD -1` fail
  alike on every machine; BET's temporary folders hold no option text and the cache key keeps
  the text a program reads in a path (`bet2/one-argument`); the replay gaps are reproduced or
  never cached (`oracle-cache verify` on the 89: 45 identical, 44 skipped with their reason). The
  smoke tier passes here in full, twice. Follow-up for the orchestrator: `oracle-cache resolve
  --check` the 45 disputed entries that verify identically now (below).
```

## 2. `CLAUDE.md`, "Where things are", the **Memory:** bullet

After "guards stop impossible sizes before allocating (ANTS-162, AFNI-75)." add:

```markdown
 The originals the harness runs (and larmorx's in-process runs of the ANTs image programs) run under a 64 GiB address-space limit, so an impossible allocation is refused alike on every machine (`validation/src/larmorx_validation/address_space.py`, 2026-10-28; random testing: twice each case's memory budget).
```

and in "Next", the words "the small part for the tests that depend on the machine," can go. In
`ORCHESTRATOR-STATE.md` (the orchestrator's own file), the line "Smoke tier on this machine: until
the test-fix part, run `pytest tests/parity -k "not allocation-refused"` ..." can go once merged.

## 3. `docs/decision-log.md` (one entry)

```markdown
### 2026-10-28: the originals run under a fixed address-space limit (orchestrator, by measurement)
Linux refuses an allocation only above about RAM plus swap, so the cases that expect ANTs to fail
to allocate (96 to 111 GiB) passed on the old laptop (94 GiB) and filled the new machine (323 GiB)
until its box killed them. Every original program the harness runs, and larmorx's in-process runs
of the ANTs image programs, now run under `RLIMIT_AS` = 64 GiB: 6.8 times the largest real call
measured (mcflirt on the 600-volume BOLD series, 9.4 GiB of address space), 3.3 times the
harness's own largest peak (19.3 GiB), 32 GiB below the smallest refusal. Random testing derives
it from each case's memory budget (twice it, never below 64 GiB): a heavy case runs under twice
its box and is never refused what the box allows. Measured first (rule D):
`validation/investigations/address-space/`.
```

## 4. `docs/validation/README.md` (the validation index), the oracle cache's paragraph

If the index summarises the cache's rules, add:

```markdown
Since 2026-10-28 the key keeps the text a program reads in a path, files named by a pattern and
paths with blanks; the cache applies ANTs' clock and memory rules to every run; and the originals
run under a 64 GiB address-space limit ([oracle-cache.md](oracle-cache.md)).
```

## Cache (for the orchestrator, after the merge)

The workspace's cache was not modified (only new entries were added by the suites' runs; the
verifies ran on copies made with `validation/investigations/oracle-cache/copy_entries.py`). The
move's verify left the 89 entries **disputed**; with this part's comparison and replay, 45 of
them verify identically and could be served again:

```bash
grep -E "^same" <workspace>/oracles/logs/machine-tests-verify89b.log | awk '{print $2}' > /tmp/same45.txt
scripts/orchestration/lxv.sh . oracle-cache resolve $(cat /tmp/same45.txt) --check \
  --reason "machine-move-proof.md, Fixed: verify starts the container with the suite's variables (Apptainer's HOME warning), compares NIfTI-2 without its extensions (AFNI's history note), masks a shell's process ID"
```

(the 38 fMRIPrep `bet` runs, the 6 3dTshift NIfTI-2 runs, BET's crash; until then those runs run
afresh, uncached). The other 44 are never reached again (re-keyed or never cached); they stay, as
everything in the cache does.
