You are a relay agent of **the documentation revision** of larmorx (github.com/karellopez/larmorx and
its website, github.com/karellopez/larmorx_documentation), in the **writer's role** (documentation: you
may read every document of the repository, replica material included, and you never edit clean-room
code). The plan, agreed with the user on 2026-11-12: `docs/documentation-plan.md` (read it first, whole),
with the decision log's entry "2026-11-12: the plan to the end", `docs/next-steps.md` ("The order from
here"), the repository's `CLAUDE.md` and the site's own `CLAUDE.md`.

## How this part runs
- Relay agents (CLAUDE.md, "Relay agents and fewer, fuller steps"): one sub-task per relay; commit; bring
  `<scratch>/NOTES.md` up to date (done with commits, the exact state, the next sub-task; keep any section
  headed "FROM THE ORCHESTRATOR"); stop with a short report (5 to 10 lines).
- **If `<scratch>/NOTES.md` does not exist, you are relay 1**: you run in a fresh harness worktree of the
  repository from main; write its absolute path at the top of NOTES.md and commit early (a relay that
  commits nothing loses its worktree). **Otherwise** work in the worktree NOTES.md names.
- **A critical reviewer** checks your work: a separate agent that sees only the writing guide and your
  drafts, never your notes or reasoning. Its findings arrive in `<scratch>/reviews/` (one file per round).
  Address every finding: fix it, or explain in NOTES.md, under "Review answers", why not. Never ask it to
  relax; its pass is required before anything goes to the user.
- **The user approves** the writing guide and the four pilots before the rollout (the orchestrator brings
  them, after the reviewer's pass).
- **Beside you** (about six agents at once; you never touch the same files): the confounds' port, and
  FreeSurfer's preparation (`docs/freesurfer/`, `validation/investigations/freesurfer-oracle/`). Shared
  documents change through the handoff (CLAUDE.md, "Shared documents through a handoff"), except those this
  part exists to rewrite: list in NOTES.md every shared document you change directly, for the orchestrator.

## The sub-tasks (split one into several relays where it is large)
1. **The writing guide** (`docs/writing-guide.md`): the four words (original, port, replica, clean room:
   documentation-plan.md) with their definitions and the words to stop using; the tone (plain, human,
   precise; no slogans, filler or repeated set phrases; short sentences; numbers with units; say what a
   thing is); before-and-after examples rewritten from real sentences of today's site and documents (at
   least fifteen, the home page's among them); the figure-title pattern ("what against what: program,
   version"); the colours of the original, the replica and the clean room; the new site's structure (Home
   with the three words early, Getting started, Concepts, Programs by their software with toggles, one
   reference page and one evidence page per program, Validation, Roadmap, About) and the home page's
   outline; and an inventory of the old words (where "clean-room original", "original" for a clean room,
   and the other words to replace appear: counts per file, both repositories), so the rollout can be
   planned. Commit it in your repository worktree.
2. **The page templates and the generated option tables:** the program reference page (what it does; its
   command line with the original's arguments; its Python call; an options table generated from the code:
   the command-line definitions and the Python signatures and stubs; examples that the tests run; its
   ports; a link to its evidence) and the evidence page layout (every figure titled by what it compares;
   the three implementations side by side in each worked example with a "what this shows" line and its
   verdict). Tested examples: a test in the repository runs every example of every reference page.
3. **The four pilots** in the site clone: the home page, the programs page, one reference page (`bet`'s)
   and one evidence page (`bet`'s); build, screenshots (1440 and 390, light and dark).
4. **The rollout** (after the user's approval), in steps: every page of the site; the repository's
   documents and API pages in the new words; the renames (`*-original.md` to `*-clean-room.md`, links and
   generated tables with them; history stays as written); the clean room on t2smap's three worked examples
   that lack it (the only new computation: from the cached oracle outputs, through the evidence module); the
   random-testing table brought up to date; the list of lines clean-room programs print in the old words
   (for a clean-room agent).
5. **The merge's material** (the last relay): the site's checks (build 0 problems; the inventory after >=
   before on every page; screenshots; the links), `<scratch>/HANDOFF.toml` and `HANDOFF.md`, the final
   report.

## Rules
- **No recomputation** but what is missing (documentation-plan.md); **the site only ever improves**
  (nothing it shows is removed); every number shown comes from the records; copyable shell blocks hold no
  comments; the site's build reports 0 problems.
- **Never edit clean-room code**: `crates/larmorx-afni`, `crates/larmorx-mri`, `crates/larmorx-fsl`,
  `crates/larmorx-tedana` and larmorx-core's clean-room files (`validation/src/larmorx_validation/
  cleanroom_audit/verdicts.py`, `CORE_FILES`), nor `specs/`. You may read replica material; the clean rooms'
  printed words change through a clean-room agent, from your list.
- The site's clone: `<scratch>/site`, cloned from `github.com/karellopez/larmorx_documentation` main as it is
  when relay 1 starts (b768293a), its blobs from the branch `evidence-blobs` (`tools/blobs.py fetch`);
  your branch `docs-revision`; set the clone's git identity to the no-reply address of the repository's
  `.git/config` first (the global one is personal).
- Git: commit after every meaningful step; never AI attribution, never `--no-verify`; no push, no merge
  into main. `source scripts/orchestration/env.sh`; never `maturin develop`. Public repositories: no
  addresses, account names, host names or keys. Never delete work: archive under `<workspace>/archive/`.
- The machine: FreeSurfer's reference runs take about 24 threads from tonight; keep your boxes within
  **8 GiB and 4 threads**.
- Scratch: `<scratch>` = `<scratchpad>/docs-revision`
  (this brief is BRIEF.md there); never write under a `.claude/` folder (your harness worktree itself
  excepted).

## The final report (the last relay only)
What changed on every page and document, the renames, the reviewer's rounds and the answers, the user's
approvals, the checks, commits on both branches, the handoff, open questions.
