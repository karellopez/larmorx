# The documentation revision: the plan

Agreed with the user on 2026-11-12 (decision log, "2026-11-12: the plan to the end"). The website
(`larmorx_documentation`) and this repository's documents are revised together: the same words,
clearer organisation, human writing, and a reference page for every program. The order of the whole
project is in [next-steps.md](next-steps.md), "The order from here".

## What the user asked for

- **Four words, used one way everywhere:**
  - **original**: the program LARMORX is compared with (FSL's `bet`, ANTs' `antsRegistration`): the
    oracle.
  - **port**: one reimplementation of an original program in LARMORX, in Rust (or in Python where the
    original is Python code that LARMORX keeps in Python, such as pybids' BIDS index). Every port is
    either a replica or a clean room, and a program can have both: FSL's `bet` has two ports, a replica
    in `larmorx-nc` and a clean room in `larmorx` (the user: "a port can be a replica in another
    language, or a clean room").
  - **replica**: a port translated from the original's source, giving its results bit for bit (ANTs'
    and ITK's ports are replicas in the main package; FSL's are in `larmorx-nc`, tedana's in
    `larmorx-gpl`).
  - **clean room**: a port written again from a description of the original's behaviour and runs of the
    original, never its source, built for speed. Never "clean-room original": "original" means the
    oracle program only.
  The home page explains the three words original, replica and clean room in an early section, with one
  concrete example; the deeper ideas (the companion packages, `fix` and `reproduce`, threads, why clean
  rooms exist, the licences) go on a Concepts page.
- **The home page in plain words** that a person would write: what LARMORX gives (many neuroimaging
  toolboxes in one package, `pip install larmorx`, no other installations, the same results on every
  computer), no slogans ("Many toolkits, one framework" goes).
- **Programs grouped by their software** (ANTs, AFNI, FSL, tedana, sdcflows and fMRIPrep, later
  FreeSurfer and Workbench), each group opening to show its ported programs and which ports exist.
- **A reference page for every program:** what it does, its command line (the original's arguments),
  its Python call, an options table generated from the code, examples that the tests run (so they never
  go stale), its ports and a link to its evidence. The repository's 38 API pages hold most of this today.
- **Evidence pages with one layout:** every figure says in its title what it compares ("clean room
  against the original: FSL `bet` 6.0.7.23"), the original, the replica and the clean room keep the same
  colours everywhere, each worked example shows them side by side with a "what this shows" line and its
  verdict.
- **The Validation page reorganised:** how LARMORX tests, what each verdict means, where every program
  stands, and the random-testing table brought up to date (today it shows the last long run, before t2smap
  and the FSL programs).
- **Writing that feels human**, consistent and intuitive across both repositories.

## Rules

- **No recomputation** (the user, 2026-11-12): the revision rearranges the evidence already computed
  (the site's JSON and images). Only what is missing is computed: today, the clean room on three of
  t2smap's four worked examples (the other programs with a clean room show it in every example).
- **The site only ever improves** (the user, 2026-10-24): nothing it shows is removed; content moves or
  becomes clearer, proved by the before/after inventory and screenshots at the merge.
- **The same words in this repository:** prose, API pages, messages and file names (`*-original.md`
  records and findings become `*-clean-room.md`, links and generated tables with them); history stays as
  written (`docs/parts/`, the decision log's old entries, commit messages).
- **The documentation agents read replica material** (the evidence texts, the findings), so they never
  edit clean-room code: where a clean-room program prints the old words, the writer lists the lines and a
  clean-room agent changes them.
- The site's build reports 0 problems; copyable shell blocks hold no comments (zsh); every number shown
  comes from the records.

## Who does it

- **The writer** drafts, from a short writing guide.
- **The critical reviewer**, a separate agent that sees only the guide and the drafts (never the
  writer's reasoning), checks every page: each number against the records; the four words; every figure
  saying what it compares; a newcomer can follow it; it sounds human (no slogans, filler or repeated
  phrases); nothing removed; links and examples work. It reports findings with suggested wording and does
  not edit; the writer fixes until the reviewer passes the page.
- **The user** reviews the writing guide and four pilot pages before the rollout.

## Steps

1. **The writing guide** (`docs/writing-guide.md`): the four words, the tone, examples of good and bad
   sentences, the figure-title pattern, the colours. The user approves it.
2. **The page templates:** the program reference page (with its options table generated from the
   command-line definitions and the Python stubs, the "generated option tables" of the former templates
   part) and the evidence page layout.
3. **Four pilots:** the home page, the programs page, one program's reference page, one evidence page.
   The user reviews them.
4. **The rollout:** every page of the site, the repository's documents and API pages, the renames, the
   three t2smap examples, the random-testing table; the clean-room programs' printed wording through a
   clean-room agent.
5. **The merge:** the inventory (after ≥ before on every page), screenshots, the site's build, the links.
