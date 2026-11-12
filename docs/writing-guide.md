# Writing guide

How LARMORX's documentation is written: the website, this repository's documents and API pages,
and the lines its programs print. The documentation revision
([documentation-plan.md](documentation-plan.md)) rewrites both repositories with it, and every
page written afterwards follows it. Draft of 2026-11-12, for the user's approval.

Every fact in this guide comes from files a clean-room implementer may read (section 1, "Who
changes what"). They are the site's pages and evidence, the validation records (all but
`tedana-t2smap-replica`), the API pages, the benchmark reports, `CHANGELOG.md`, the main
package's Python and command-line code, and the clean-room audit's restriction table and lists.
Nothing comes from the restricted material. Section 4 quotes four comparison labels from
3dvolreg's and t2smap's evidence, which restricted modules export (section 1): they name the
runs compared, with no fact of the original's source.

## 1. Four words

| Word | What it means | Example |
|---|---|---|
| **original** | The program or function that a port reimplements, as its own developers release it. Every port is compared with its original. | `bet` from FSL 6.0.7.23; `antsRegistration` from ANTs 2.6.5; fMRIPrep 25.2.6's resampler |
| **port** | One reimplementation of an original in LARMORX: in Rust, or in Python where the original is Python code that LARMORX keeps in Python (pybids' BIDS index, sdcflows' workflows). Each port is one kind, a replica or a clean room, so a program can have two ports. | `bet` has two ports, a replica and a clean room; `antsRegistration` has one, a replica. |
| **replica** | A port translated from the original's source, step by step, to give the original's results bit for bit. Where a record shows a difference, it says why (most often the C library's rounding of `exp` or `pow`). A replica comes under its original's licence terms, so it lives in the package for those terms: tedana's LGPL-2.1 code is under GPL-3.0, which the LGPL allows. | `antsRegistration`'s replica in `larmorx`; `bet`'s in `larmorx-nc` |
| **clean room** | A port written again without the original's source or its replica: from a description of what the original does, its documentation, papers and runs of the original. It is built for speed and carries LARMORX's licence, Apache-2.0. | `bet`'s and `3dvolreg`'s clean rooms in `larmorx` |

Where each port lives on `main` (section 2, rule 14):

| Software | Replica | Clean room |
|---|---|---|
| ANTs and ITK | `larmorx` | planned, after FreeSurfer |
| AFNI | `larmorx` (with AFNI's CC BY 4.0 credit) | `larmorx` |
| FSL | `larmorx-nc` (non-commercial use only) | `larmorx` |
| tedana (`t2smap`) | `larmorx-gpl` (GPL-3.0) | `larmorx`, from the next release (0.1.0rc1 has only the replica) |
| sdcflows, fMRIPrep's steps and the Python libraries they call | `larmorx` | planned, after FreeSurfer |

How to use them:

- **original** never names a LARMORX program. Give the version where it matters: "FSL 6.0.7.23".
- **clean room** is a noun of two words: "the clean room", "`bet`'s clean room", "clean rooms".
  Before another noun it takes a hyphen: "clean-room code", "the clean-room audit". Never
  "clean-room original", "clean-room implementation" or "clean-room port".
- **replica** stands alone. Not "exact replica" or "bit-exact replica": the word already says it.
  Where the reader needs the fact, give it with its number: "the replica gives FSL's files byte
  for byte in 201 of its 263 cases".
- **port** is the word for both kinds. Never make it a third kind beside them: ANTs' programs have
  replicas, not ports "from the source".
- In code the names are `implementation="auto" | "replica" | "clean-room"` and
  `--implementation clean-room`. On the command line LARMORX's options stand between `larmorx`
  and the family: `larmorx --implementation clean-room fsl bet ...`. Everything after the
  program is the original's own arguments.
- The published release, 0.1.0rc1, spells the clean room `"original"`, and `main` still accepts
  that spelling. A page that shows the value gives `"clean-room"`, with `"original"` (0.1.0rc1)
  in brackets.
- What LARMORX has that is not a port is **LARMORX's own**: its NIfTI reader and writer, the
  `larmorx` command itself. The program LARMORX's own work is compared with is called by its name
  (nibabel), never *the original*.

### Other words, one meaning each

| Write | Not | Note |
|---|---|---|
| the original; a run of the original | the oracle; the upstream | Pages users read. Developer documents keep "oracle" and "upstream", and so do names (the oracle-output cache); question 5. |
| the comparison with the original; the record | parity; the parity tests | Same note. |
| the quick check, the standard comparison, the full comparison | the smoke, standard and full tiers | Say what runs and when; the names *smoke*, *standard* and *full* once, in italics, after the explanation. |
| compared cases | | The cases both programs complete (neither refuses nor crashes). Say it once per page, with the whole: "198 of its 200 compared cases, out of 252". |
| byte for byte (files, headers included); bit for bit (values) | bit-exact | *bit-identical* only as the verdict's name. |
| the original's own variability | the original's spread; the original's own spread | Also the figure's name (section 4). |
| the cross-platform checks | the golden tests | On user pages. |
| a random-testing run of half an hour | a trial night; a night | Give the duration. |
| a defect of the original, corrected by default | D13 | The register's ids (ANTS-40, ITK-50) stay, linked to the register's summary table. |
| `fix` (the default) and `reproduce` | default mode; reproduce mode | The values of `upstream_defects` and `--upstream-defects`. |
| the software (ANTs, FSL) in general; *a project* when counted ("the six projects LARMORX ports from") | the ecosystem; the toolkit; "each software" | In prose. Not *package* for a project: LARMORX's packages are `larmorx`, `larmorx-nc` and `larmorx-gpl`. |
| the family (`fsl` in `larmorx fsl bet`) | | Only for the command line's first word after the options. On user pages never for a group of licences: say "FSL's licence". |
| program; function (a Python call with no command line) | tool | "tool" only in names that exist (`validation/tools.toml`). |
| port; the replica; the clean room | implementation | `implementation` stays the option's name. |
| `larmorx` (the package); LARMORX (the project) | the main package; LARMORX itself | Developer documents keep "the main package"; question 5. |
| `larmorx-nc`, `larmorx-gpl` (the companion packages) | the companion wheels | Say what each holds and its licence. |

### Where the old words stay

- **History**, as written: `docs/parts/`, the decision log's entries before 2026-11-12,
  `docs/status-log.md`, the released sections of `CHANGELOG.md`, commit messages, and
  `validation/investigations/` and `validation/probes/` (the tools and outputs behind past
  findings).
- **Identifiers**: random testing's case names and descriptor `NAME`s, and the record, suite and
  cache names written inside records until a record is made again (Mechanics, below). Code keeps
  accepting `"original"`.
- **Pages made by a measurement**: the records (the pages of `docs/validation/` that a comparison
  writes, with their `.json`), the benchmark reports (`docs/benchmarks/`, its README aside) and the
  clean-room audit's page (`docs/validation/clean-room-audit.md`). A comparison suite, a
  benchmark or the audit writes each, except 10 of the 21 benchmark reports, written by hand from
  their script's measurements. The evidence cites the records and the reports by path and SHA-256
  (`provenance.records`, `provenance.benchmarks` in `static/evidence/*.json`). So nobody changes
  them afterwards, not even their words:
  - each keeps its words until it is made again in its normal course, as the final validation
    phase does; none is made again only for its words;
  - the new words go into what writes them (the suites' and the benchmarks' report code, the
    evidence module, `tables.py`, the audit's summary), so the next ones use them;
  - where the site shows a record's text, the evidence module may reword it there;
  - a renamed record or report keeps its contents byte for byte, and the evidence exported again
    names its new path with the same SHA-256.

### Who changes what

- **The writer** changes the site, the documents, the API pages and the evidence module's texts.
  In code, the writer changes the Python package's messages and docstrings, and LARMORX's own
  lines in `larmorx-cli` and the replica crates; never the original's messages, which a replica
  prints bit for bit.
- **A clean-room agent** changes the clean rooms' code and the lines they print
  (`crates/larmorx-afni`, `-mri`, `-fsl`, `-tedana` and larmorx-core's clean-room files), from
  the writer's list. **A spec writer** changes `specs/`. **The orchestrator** changes
  `CLAUDE.md`.
- **Files a clean-room implementer may read.** `docs/porting-guide.md` §1 says what an
  implementer may read, and the clean-room audit's restriction table
  (`validation/src/larmorx_validation/cleanroom_audit/restrictions.py`) applies it. For this
  rule, treat every file as readable except the restricted material, which the table lists:
  - the originals' source that it restricts (today FSL's, AFNI's, tedana's, FreeSurfer's,
    Workbench's, MSM_HOCR's, glibc's and libstdc++'s);
  - the replica crates (`crates/larmorx-afni-replica`, `crates-gpl/`, `crates-nc/`);
  - the findings written from the source: the thirteen files §1 lists, by name, since two of them
    (`afni-tshift.md`, `fsl-mcflirt.md`) say so in other words than "Restricted";
  - the register's restricted sections, headed `## Restricted: ...` (their entries carry † in the
    summary table, which anyone may read);
  - `docs/parts/`, the probes in `validation/probes/`, the session transcripts and other agents'
    scratch notes;
  - what the clean-room briefs forbid, as the audit applies it (`cleanroom_audit/verdicts.py`,
    `BRIEF_RESTRICTED` and `BRIEF_FILES`, rules R15 and R16):
    - the replica authors' random-testing cases and their entries in
      `validation/difftest/triage.toml`;
    - the replicas' comparison suites, random-testing descriptors and evidence modules (files of
      `parity/`, `evidence/` and `difftest/tools/` named `*replica*`, `fsl_mcflirt*` or
      `afni_volreg.py`, and `evidence/` and `difftest/tools/` `tedana_t2smap.py`);
    - the investigations of replica parts and spec writers that `BRIEF_FILES` lists;
    - `t2smap`'s replica record (`docs/validation/tedana-t2smap-replica.md` and `.json`) and its
      cases (`validation/difftest/cases/tedana-t2smap/`);
  - the site's pages, or sections of them, that those evidence modules export (today 3dvolreg's
    and t2smap's): restricted for the clean-room implementers of that software, through their
    briefs (the orchestrator, 2026-11-12). Clean-room implementers work from the specs and runs
    of the original, never from their software's site pages.

  This widens what the writer guards, never what an implementer may read. The writer has read
  restricted material, so in a readable file:
  - a changed sentence keeps its facts, and only its wording changes;
  - a new sentence, table row or page holds only facts already in a readable file (a record, the
    evidence's JSON, an API page, a spec, the original's own documentation), and names or links
    that file;
  - nothing comes from the restricted material, even reworded or summarised.

  New facts from restricted material go only into restricted material. Text that a restricted
  file exports or prints where anyone reads it (`evidence/afni_volreg.py` and
  `evidence/tedana_t2smap.py` write the site's JSON) is readable once exported. In such text the
  writer changes the wording only, as in any readable file. Each changed readable file is listed
  for the orchestrator, and the reviewer checks every new sentence against the readable file it
  comes from.
- **A program's reference page is readable material.** The exporter writes it only from readable
  files (section 6, "One source"), so clean-room implementers may read it. A section of today's
  pages whose facts come from restricted material stays on the evidence page, and the reference
  page links to it. Today that is `bet`'s "The standard images the variations read" (`#data`),
  which cites `docs/findings/fsl-bet.md`.

### Mechanics of the rollout

- **Shared documents** (CLAUDE.md's list). The writer edits directly those whose words this
  revision rewrites: `README.md`, `docs/licensing.md`, `docs/validation/README.md`,
  `docs/validation/golden.md`, the register and `docs/api/upstream-defects.md`, and on the site
  `site.yml` and `content/validation.md`. Each is listed in the part's notes, and the
  orchestrator checks it again at the merge. This is the documentation revision's exception to
  CLAUDE.md's handoff rule (the orchestrator, 2026-11-12). `docs/next-steps.md`, the decision log
  and `CLAUDE.md` change through the handoff.
- **Generated tables and files** (the README's tool table, the validation index's records table,
  the site's `data/status.yml`, `static/evidence/summary.json`) change through their source
  (`validation/tools.toml`, the evidence module) and the generator, never by hand. The pages made
  by a measurement keep their words until they are made again (above).
- **A printed line changed** (a message, a usage text, a `--help`): the golden cases that hash
  the output (`tests/golden/registry.py`: 3dTshift's and 3dvolreg's, among others) are recorded
  again with `scripts/record_golden.py` from a clean commit, after the quick check. Whoever
  changes the line records them.
- **Renamed files**: records, findings, benchmark reports, tests and validation modules named
  `*-original*` become `*-clean-room*` (`*_clean_room*` for Python modules), with their links,
  their record ids in `validation/tools.toml` and the generated tables. A renamed record or
  report keeps its contents byte for byte. Random testing's cases and the investigations keep
  their names. A file that a page made by a measurement links to keeps its name until that page
  is made again. Today these are five: `docs/validation/afni-volreg-original.md` (the record),
  `docs/findings/fsl-maths-original.md`, `docs/findings/fsl-prelude-original.md`,
  `docs/validation/tedana-t2smap-original.md` and `docs/benchmarks/tedana-t2smap-original.json`,
  which the benchmark reports of 3dvolreg, fslmaths, prelude and t2smap link. They are renamed
  when those reports are made again. The inventory marks them "renamed later".
- **Renamed identifiers**: a renamed comparison suite (`afni-volreg-original` becomes
  `afni-volreg-clean-room`) keeps its old name as an accepted alias, so the "Generated: with
  `python -m larmorx_validation parity afni-volreg-original`" lines of today's records still run
  until each record is made again. Random testing's descriptor `NAME`s
  (`NAME = "afni-volreg-original"`, used in `validation/difftest/triage.toml` and the cases) stay
  as they are, even where the module's file is renamed.
- **The evidence exported again** changes only texts, marks, the provenance's paths and the
  exporter's commit, which the provenance records. It also adds the fields that name each
  panel's and comparison's port and the order they are shown in (`port`, `order`, `shows`; the
  evidence module's `words.py` says how). Every blob stays byte for byte
  (`static/evidence/blobs.sha256` unchanged, checked before the push), and every SHA-256 the
  provenance cites stays the same. The lists of panels and comparisons keep their order in the
  JSON, since the site's number check and the merge's inventory find a number by its place. The
  JSON files change, and the merge's site inventory shows each change for the merge to judge.

[validation/investigations/docs-revision/inventory.md](../validation/investigations/docs-revision/inventory.md)
counts every old word, file by file (section 7).

## 2. The tone

1. **Write as a person tells a colleague.** Plain words, the program as the subject, active
   voice: "`bet` finds the brain in a head image", not "brain extraction is performed".
2. **One idea per sentence.** Most sentences under 25 words. A semicolon or a bracket only where
   it helps, never one bracket inside another.
3. **What the reader gets comes first; history comes last.** A lede says what the program does in
   one sentence, then how good each port is today, in a sentence or two per port. The status goes
   to the facts line under the lede, the dates of changes to the page's History section.
4. **Say what a thing is.** "The standard comparison: every option and fMRIPrep's own calls",
   not "the standard tier".
5. **Numbers with their unit and their whole.** "198 of 200 compared cases", "3.0 s", "307 MiB",
   "10 to 15 times faster on one thread". Every number comes from a record, linked once.
6. **No slogans, no filler, no praise.** Leave out *established*, *aims to*, *aims at*, *robust*,
   *seamless*, *powerful*, *state-of-the-art*, *comprehensive*, *framework*, *toolkit*,
   *leverage*, *simply*, *just*, *note that*, *in order to*. A phrase repeated on every page
   ("every option, message, exit status and failure case") is said once, on the Validation page,
   and linked.
7. **Claim what the record shows, no more.** "Gives FSL's files byte for byte in 201 of its 263
   cases", not "is identical to FSL". Where a port is not exact, the lede says so.
8. **British English** (behaviour, colour, licence as a noun and license as a verb), as the site's
   `en-GB`. Dates as 2026-11-12.
9. **Names.** LARMORX in prose and `larmorx` in commands; software in plain text (FSL, AFNI);
   programs, options, files and values in code (`bet`, `-f 0.2`, `"clean-room"`). A version goes
   with its software: "`bet` from FSL 6.0.7.23" in prose, "FSL `bet` 6.0.7.23" in titles and
   labels.
10. **Licences in plain words.** "Non-commercial use only"; "whoever passes it on must pass on its
    source under the same licence".
11. **Copyable shell blocks hold commands only**: no comments (zsh, macOS's shell, reads `#` as
    an argument), no prompts, every line runnable as it stands. Every example is run by the tests
    (the reference page's template, step 2 of the plan, says how an example gets its input).
12. **Link instead of repeating**, and explain a term at its first use on a page or link Concepts.
13. **Shorten words, never facts.** A rewrite keeps every fact of the sentence it replaces, on the
    same page or on the page it links to (History, Concepts, Validation). The reviewer checks each
    rewritten sentence's facts against the old one.
14. **Pages describe `main`**, which the tests run. What 0.1.0rc1, the published release, lacks
    is marked where a page first shows it, "from the next release", with 0.1.0rc1's way beside it
    (as the installation page's "Coming with the next release" box does). Question 4.

## 3. Before and after

Real sentences of today's site and documents, rewritten. Numbers in the rewrites come from the
same records as the sentences they replace.

**The home page** (`content/index.md` of the site)

1. Before (the end of the hero): "Many toolkits, *one* framework. Spins of five software
   ecosystems, precessing at one Larmor frequency, gather into one brain. Each keeps its colour."
   After: "Spins in resonance. Five groups of spins, one colour each for ANTs, AFNI, FSL,
   FreeSurfer and Workbench, come to one frequency and gather into a brain from the MNI template."
   Why: the slogan goes; the text says what the animation shows.
2. Before: "LARMORX is an open-source, cross-platform neuroimaging toolbox that reimplements
   established neuroimaging algorithms and command-line tools in Rust. It aims to reproduce the
   functionality and behaviour of tools from ANTs, AFNI, FreeSurfer and others, without requiring
   you to install the original binaries or manage complex system dependencies."
   After: "LARMORX puts programs from ANTs, AFNI, FSL, tedana, sdcflows and fMRIPrep in one Python
   package. `pip install larmorx` is all it takes on Linux, macOS and Windows: no other
   installation, and the same results on every computer, which the tests check on six platforms.
   tedana's `t2smap` comes with the next release; in 0.1.0rc1 it is in `larmorx-gpl`."
   Why: what the reader gets, in their words; no *established* or *aims to*; FreeSurfer is not
   ported yet, so it is not named; what 0.1.0rc1 lacks is marked (rule 14).
3. Before: "Each tool is reimplemented in one of three ways, chosen by the licence of the
   original. Where a licence requires it, algorithms are reimplemented under clean-room
   development practices, so that restrictively licensed source code is not incorporated."
   After: "Most of LARMORX's programs are ports of original programs. Each port is a replica or a
   clean room, and some programs have both. The original's licence decides which package each
   port goes in."
   Why: two kinds of port, not three ways; the set phrase about practices goes.
4. Before: "Ported from the source. Tools with permissive licences, ANTs and ITK (Apache-2.0),
   are ported from their source, with each Rust function's origin recorded in the crate's
   `PROVENANCE.md`."
   After: "Replica. The original's source translated step by step into Rust, or, for some Python
   originals, rewritten step by step in Python, to give its results bit for bit. A replica comes
   under its original's licence terms: the replicas of ANTs, ITK, AFNI, sdcflows and fMRIPrep
   are in `larmorx`, FSL's in `larmorx-nc`, and tedana's `t2smap` in `larmorx-gpl`, under
   GPL-3.0. Each Rust function's origin is recorded in its crate's `PROVENANCE.md`."
   Why: ANTs' ports are replicas; every replica's package is named; the provenance fact stays.
5. Before: "A clean-room original. Where the licence is restrictive (FSL, tedana, Workbench), an
   original is written from a behavioural specification, documentation and runs of the original
   program, never from its source."
   After: "Clean room. Written again from a description of what the original does, its
   documentation and runs of it, never from its source, and built for speed. It carries LARMORX's
   licence, Apache-2.0, so it is in `larmorx`: AFNI's and FSL's programs have one, and tedana's
   `t2smap` from the next release."
   Why: *original* named two things in one sentence; Workbench has no port yet.
6. Before: "Tools with both take `implementation="auto" | "replica" | "original"`."
   After: "A program with both ports takes `implementation="replica"` or `"clean-room"`
   (`"original"` in 0.1.0rc1). `"auto"`, the default, runs the replica when it is installed
   (AFNI's are built in) and the clean room otherwise."
   Why: the value is `"clean-room"` since the easy command line; 0.1.0rc1's users still need
   theirs; and what `auto` does in both cases.
7. Before: "In magnetic resonance, spins of one species share one frequency. LARMORX would like the
   same for neuroimaging tools: many ecosystems, one shared objective."
   After: "The name comes from the Larmor frequency: in a magnetic field, spins of one kind precess
   at one frequency. LARMORX brings programs from different projects into one package and
   compares each with its original the same way."
   Why: the metaphor's slogan goes; the sentence says what is shared.
8. Before (the text beside the equation): "In the same way, LARMORX aims to bring neuroimaging
   tools developed in different software ecosystems into a common computational framework,
   aligned toward a shared objective: **FAIR** scientific research, research that is Findable,
   Accessible, Interoperable and Reusable."
   After: "LARMORX brings programs from different projects into one package for one purpose:
   **FAIR** research, research that is Findable, Accessible, Interoperable and Reusable."
   Why: no *aims to* or *common computational framework*; the FAIR list below it says what each
   letter means for LARMORX.

**Other pages of the site**

9. Before (`content/tools/index.md`): "Each page says what the tool is, how LARMORX implements it
   (a port of the source, a clean-room original, an exact replica) and how to call it, then shows
   its validation record in figures."
   After: "Each program has two pages: its reference (what it does, its command line, its Python
   call, its options and examples) and its evidence (how its ports compare with the original, in
   figures)."
10. Before (`content/validation.md`): "**The original program is the oracle.** Each LARMORX tool is
    run beside the program it reimplements, on the same files with the same arguments, and their
    outputs, exit statuses and messages are compared."
    After: "**Every port runs beside its original**, on the same files with the same arguments.
    Their outputs, exit statuses and messages are then compared."
11. Before (`content/validation.md`): "**Three ways of reimplementing.** A *port* translates source
    whose licence allows it (ANTs, ITK) and aims at the original's bits. Where the licence
    restricts the source (FSL, Workbench), there are two: an exact *replica* in a separate package
    under the original's licence (`larmorx-nc` for FSL), which aims at the original's bits, and a
    *clean-room original* in LARMORX itself (Apache-2.0) ..."
    After: "**Two kinds of port.** A replica is translated from the original's source and gives its
    results bit for bit; where it does not, its record says why. A clean room is written without
    the source: it gives the original's results within stated thresholds, often bit for bit, and
    is built for speed. ANTs' programs have replicas; AFNI's and FSL's programs have both, and so
    does tedana's `t2smap` from the next release."
12. Before (`content/validation.md`): "**Parity tiers.** Each tool's suite of cases runs in tiers:
    *smoke* under pytest, *standard* for the records quoted on this site (fMRIPrep's, sMRIPrep's
    and sdcflows' own command lines on real data, every option, generated edge cases, the invalid
    inputs), *full* where a tool has more."
    After: "**Three sizes of comparison.** A quick check, a few cases of every program, run with
    the test suite before every change goes into the published code (*smoke*). The standard
    comparison, which this site shows: fMRIPrep's, sMRIPrep's and sdcflows' own command lines on
    real data, every option, generated edge cases and invalid inputs (*standard*). And, for some
    programs, more and larger cases (*full*)."
    Why: what each size holds, every fact kept; "at every merge" (the same page's table) in plain
    words.
13. Before (`content/tools/fsl-bet.md`, the lede, 137 words): "FSL's brain extraction (bet, bet2)
    and its skull and scalp surfaces (betsurf), in two implementations: a clean-room original in
    LARMORX (Apache-2.0), experimental, written from a behavioural specification without FSL's
    source, whose bet2 and bet give FSL 6.0.7.23's files bit for bit in nearly every case ... and
    since 2026-11-05 so does the clean-room original, every program its own clean-room program."
    After: "`bet` finds the brain in a head image; `betsurf` adds the skull and the scalp. The
    replica (`larmorx-nc`) matches FSL 6.0.7.23's exit status, messages and files, byte for byte,
    in 201 of its 263 cases, and 47 more are rejected by both. Of the other 15, FSL crashes in 13
    and reads memory beyond the image in one; the last needs an FSL program that LARMORX does not
    replicate yet. The clean room (`larmorx`) is within the thresholds in 198 of its 200 compared
    cases. Its brains, masks and outlines are FSL's bit for bit in 378 of 380 images, and
    `betsurf`'s masks and outlines are within the thresholds in 114 of 116, 34 of them bit for
    bit."
    Why: what it does, then a sentence or two per port, with where each is not complete or not
    exact (the replica's last case, the clean room's two cases and `betsurf`). The records count
    cases and images apart, so the lede names both units. "Experimental" goes to the facts line
    (example 14), how each port was written to "Its ports" and the dates to the page's History.
    The numbers: [fsl-bet-replica.md](validation/fsl-bet-replica.md) (its summary and its
    expected divergences) and [fsl-bet.md](validation/fsl-bet.md).
14. Before (`content/tools/fsl-bet.md`, the facts): "Status: experimental (original) · validated
    (replica)".
    After: "Status: replica validated; clean room experimental."
15. Before (`content/tools/fsl-bet.md`): "The original's record runs the replica's standard cases
    against FSL 6.0.7.23, betsurf's real heads and, since 2026-11-05, the variations,
    standard_space_roi and fslroi (the record, 252 cases): 198 of its 200 compared cases are within
    the thresholds, 46 more rejected by both alike and 6 where FSL crashes and `reproduce` fills
    what FSL leaves undefined."
    After: "The clean room's record compares it with FSL 6.0.7.23 in 252 cases: the replica's
    cases, real heads for `betsurf`, and `bet`'s options with `standard_space_roi` and `fslroi`
    (since 2026-11-05). 198 of the 200 compared cases are within the thresholds. 46 more are
    rejected by both alike. In 6, FSL crashes, and `reproduce` fills in what FSL leaves undefined."
16. Before (`site.yml`, N4's summary): "Bias-field correction, as fMRIPrep, sMRIPrep and sdcflows
    call it. Port."
    After: "Bias-field correction, as fMRIPrep, sMRIPrep and sdcflows call it. Replica."
17. Before (`content/roadmap.md`): "**Clean-room originals of the permissive ports** | A clean-room
    original, built for speed, of every tool ported from permissively licensed code ..."
    After: "**Clean rooms for the programs that have only a replica** | A clean room, built for
    speed, of every program that has only a replica: ANTs and ITK, sdcflows' field maps,
    fMRIPrep's resampler and Python steps, nitransforms, the confounds."
18. Before (`data/verdicts.yml`, a verdict's name): "Deliberate correction (D13)".
    After: "Deliberate correction". Why: D13 is a decision's number; the Validation page says what
    the verdict means and links the register.

**The evidence** (`static/evidence/fsl-bet.json`, written by the evidence module)

19. Before (a difference panel): "original − FSL 6.0.7.23", above it "larmorx against the
    original".
    After: "clean room − FSL `bet` 6.0.7.23", above it "the clean room against the original".
    Why: in the old caption *original* named LARMORX's program in one line and FSL's in the next.
20. Before (an image panel): "larmorx's clean-room original", below it "default mode (Apache-2.0,
    crate larmorx-fsl), against FSL 6.0.7.23".
    After: "clean room", below it "`larmorx`, Apache-2.0 · `fix` (the default) · against FSL `bet`
    6.0.7.23".
    Why: the package, the licence and what it is compared with stay; the crate goes to the
    reference page's "Its ports".
21. Before (the speed chart): "Seconds for the whole command (read, compute, write), median of
    repeated runs, log scale; the factor is the clean-room original's speed-up over FSL at one
    thread".
    After: title "Speed, both ports against the original: FSL `bet` 6.0.7.23"; caption "Seconds for
    the whole command (reading, computing, writing), the median of repeated runs, on a log scale.
    The factor is how many times faster the clean room is than FSL, both on one thread."
22. Before (an agreement chart): "larmorx clean-room original, against FSL 6.0.7.23 (standard
    tier)".
    After: "Agreement, clean room against the original: FSL `bet` 6.0.7.23 (standard comparison,
    252 cases)".
23. Before (a worked example): the title "fMRIPrep's skull strip of a real BOLD reference" and no
    summary line.
    After: the same title, then "What this shows: on fMRIPrep's own call, the replica and the clean
    room both give FSL's brain and mask, every voxel." with the verdict *bit-identical*.

**This repository**

24. Before (`README.md`): "Where a licence requires it, algorithms are reimplemented under
    clean-room development practices, so that restrictively licensed source code is not
    incorporated."
    After: "Where the original's licence keeps its code out of an Apache-2.0 package, LARMORX
    writes a clean room: a new program built from a description of what the original does, never
    from its source."
25. Before (`README.md`, the packages): "`larmorx` | every tool below: the ports, the clean-room
    originals and the AFNI replicas".
    After: "`larmorx` | the replicas (ANTs and ITK, AFNI, sdcflows, fMRIPrep's steps and the Python
    libraries they call), the clean rooms (AFNI, FSL, and tedana's `t2smap` from the next release)
    and LARMORX's own NIfTI reader and writer".
26. Before (`docs/api/fsl-bet.md`): "**Two implementations.** ... the bit-exact **replica**,
    `larmorx-nc fsl bet|bet2|betsurf` ..., and the clean-room **original** in the main package
    (Apache-2.0, `crates/larmorx-fsl`), written from `specs/bet.md`, the BET paper and black-box
    runs of FSL".
    After: "**Two ports.** The replica, `larmorx-nc fsl bet|bet2|betsurf`, in `larmorx-nc` (FSL's
    licence: non-commercial use only), translated from FSL's source; and the clean room, in
    `larmorx` (Apache-2.0, `crates/larmorx-fsl`), written from `specs/bet.md`, the BET paper and
    runs of FSL."
    Why: a wording change in a page a clean-room implementer reads: every fact and name was in the
    sentence already (section 1, "Who changes what").
27. Before (`docs/overview.md`, the words): "**Upstream** | An original program we reproduce";
    "**Clean-room original** | Our own code, written by someone who never sees the upstream source
    (or its replica)."
    After: "**Original** | The program a port reimplements, as its developers release it";
    "**Clean room** | A port written by someone who has never seen the original's source or its
    replica."
28. Before (a line `larmorx afni 3dvolreg -help` prints, from a clean room's code): "Usage: 3dvolreg
    [options] dataset   (larmorx: clean-room original of AFNI 26.3.05's 3dvolreg)".
    After: "Usage: 3dvolreg [options] dataset   (larmorx: the clean room of AFNI 26.3.05's
    3dvolreg)". Who: a clean-room agent, from the writer's list; the golden cases that hash it are
    recorded again.

## 4. Figures and their titles

**The pattern:** *what it shows*, *A* against *B*: *software* `program` *version*.

- *A* and *B* are: the original; the replica; the clean room; both ports; **fMRIPrep's output**
  (the file fMRIPrep wrote, in its own run); **the known answer** (what a synthetic input was
  made to give: the true transform, the exact field); **a reference computed another way** (the
  Jacobian factor on the BOLD's own grid, where both methods agree); **the input**; **the
  original's starting point** (a registration's initial alignment).
- A comparison of something with itself says what changes, after a colon: "the original against
  itself: two versions", "the replica against itself: `fix` and `reproduce`", "the clean room
  against itself: 1 and 8 threads".
- The clean room compared with the replica: "the clean room against the replica".
- LARMORX's own work is compared with a program named by its name: "LARMORX's own reader against
  nibabel".
- The title names the version of the original compared with. The caption below says what the
  axes, colours and units are, and on what data.

| Figure | Title |
|---|---|
| agreement chart | Agreement, clean room against the original: FSL `bet` 6.0.7.23 |
| speed chart | Speed, both ports against the original: FSL `bet` 6.0.7.23 |
| the original's own variability | Variability, the original against itself: FSL `bet` 6.0.7.23, run again and in fMRIPrep 25.2.6's image |
| the step between two versions | Changes, the original against itself: FSL `bet` 6.0.7.17 and 6.0.7.23 |
| random testing | Random testing, both ports against the original: FSL `bet` 6.0.7.23 |
| case table | Every case, replica against the original: FSL `bet` 6.0.7.23 |
| difference panel | clean room − FSL `bet` 6.0.7.23 (always *A* − *B*, LARMORX's port first) |
| image panel | its name ("FSL `bet` 6.0.7.23", "replica", "clean room"); a second line: its package and licence, how it ran (`fix` or `reproduce`, threads, build) and what it is compared with |

**Today's comparison labels and the new ones.** The evidence's difference panels carry a label of
what they compare (`role_label`, 854 in all). Each becomes one of the forms above; where today's
label says "larmorx", the evidence module names the port.

| Today (how many) | New |
|---|---|
| larmorx against the original (289) | the replica against the original; the clean room against the original |
| larmorx against itself (306) | the replica (or the clean room) against itself: `fix` and `reproduce` / 1 and 6 threads; the clean room against the replica (where the label says "original − replica") |
| the original against itself (120) | the original against itself: what changes (fMRIPrep's build, the repository's build) |
| larmorx against fMRIPrep (65) | the replica (or the clean room) against fMRIPrep's output, where B is fMRIPrep's file; against the original: fMRIPrep's build, where B is the original in fMRIPrep's image ("fMRIPrep's AFNI") |
| the original against itself: the version step (26) | the original against itself: two versions |
| larmorx's correction (9); larmorx's corrections (3) | the replica (or the clean room) against itself: `fix` and `reproduce` (the defect's id, ITK-50) |
| larmorx against the original (both implementations' reference) (5) | both ports against the original |
| against the truth (5); the original against the truth (1) | the original, the replica or the clean room against the known answer |
| against the reference (4) | `resample-series` (2): the replica in `fix`, and in `reproduce` (fMRIPrep's factor), against a reference computed another way: the factor on the BOLD's own grid; `sdcflows-fieldmap` (2): the original (or the replica) against the known answer: the exact field |
| what the registration changed (3) | the original against its starting point: what the registration changed |
| the original against itself: the machine (3); … the machine and numpy's kernels (3); … fMRIPrep's AFNI 25.2.09 in its image (1); … the same volumes stored another way (1) | as they are (they already follow the pattern) |
| the defect, in AFNI (2) | the original against itself: the defect (`-peels 0` and `-peels 1`) |
| the corrected result (2) | the replica against the original: `fix`, on the usual encoding; the clean room against itself: two encodings |
| the original with and without -S (2); … -B (2) | the original against itself: with and without `-S` (`-B`) |
| the same data, two encodings (1) | the input against itself: two encodings |
| the two implementations of the corrections (1) | the clean room against the replica: both `fix` |

**A worked example** has a title that names the data and the call ("fMRIPrep's skull strip of a
BOLD reference"), then one line, "What this shows: ...". Beside it stands, for each port, the
verdict of the comparison that matters most: the port against the original's newest version.
Its panels go in one order:

1. the input, and its known answer for a synthetic input;
2. the original: each version, then its other builds (fMRIPrep's build, correctly rounded maths);
3. fMRIPrep's own output;
4. the replica, `fix` then `reproduce`;
5. the clean room, `fix` then `reproduce`.

The differences follow in the same order, each with its verdict and the reason for it.

**One order everywhere:** the original, the replica, the clean room, in panels, legends, tables
and text.

## 5. Colours

| Who | Light theme | Dark theme | Its line | Its point |
|---|---|---|---|---|
| the original | ink `#4a453e` | `#b6b0a3` | solid | a square |
| the replica | blue `#467dea` | `#5890fa` | long dashes | a triangle |
| the clean room | apricot `#c76717` | `#d97530` | short dashes | a circle |

These are the colours the evidence's charts already use (`static/js/evidence.js`, `PAL`), so no
image has to be drawn again. A point is filled at one thread and open at more, as the speed
charts' dots and rings are now. Where one port appears in `fix` and in `reproduce` in one chart,
`reproduce` takes a thinner line and a smaller point. The legend names both (tedana's speed
chart, the only such chart today). Where the three lines coincide, as they do when the ports are
bit-identical, the dashes let the original's line show through.

- **One token set** for the three, read by both the CSS and the page's script
  (`--port-original`, `--port-replica`, `--port-clean-room`, light and dark). Today the line
  charts and panel labels take the clean room's colour from `--deep-larmorx`, which is `#f2b28a`
  in the dark theme; the series and speed charts take `#d97530` from `PAL`. The token set gives
  `#d97530` everywhere.
- **Everything else in a chart:**
  - every run of the original, in any version or build (fMRIPrep's build, correctly rounded
    maths) and fMRIPrep's own output, which the original made in fMRIPrep's run: the original's
    colour, with a second line that names the run;
  - LARMORX's own work (the NIfTI reader): the clean room's colour and line, labelled "LARMORX's
    own". Apricot then means code LARMORX writes itself; LARMORX's own never shares a chart with a
    clean room;
  - inputs, known answers and references computed another way: the grey of `PAL.input`
    (`#8a847a`, dark `#7d776d`), dotted, with no point;
  - the panel role `reference` (54 panels today) holds both kinds: runs of the original (with
    correctly rounded maths; FSL on the same map without its NaN) take the original's colour,
    and the computed reference (`resample-series`) and the known answer (`sdcflows-fieldmap`'s
    exact field) the grey. The evidence module names which each panel is.
- **A port's colour always comes with its name** (a legend, a caption) and its mark, never alone.
- **Ports are lines, marks and labels; verdicts are filled areas.** A verdict's colour always comes
  with its pattern and its name (`data/verdicts.yml`). Three verdicts share a hue with a port:
  *bit-identical* (solid) and *identical with correctly rounded maths* (hatched) the apricot,
  *within the original's variability* the blue. *Rejected by both* (hatched) shares its grey with
  the inputs. A chart coloured by verdict names the port in its title.
- **The difference maps keep their scale**: blue below zero, apricot above, with the colour bar
  that says so. 81 posters are drawn with it, and redrawing them is not part of the revision.
- **The software colours** (`data/ecosystems.yml`) appear only as pastels: a band behind a
  software's group on the Programs page and a chip with its name. Never its deep colour as a dot
  beside a program's name, as today (`.dot` takes `--kd`): ANTs' deep colour is the replica's blue,
  LARMORX's the clean room's apricot.
- **tedana, sdcflows and fMRIPrep get a pastel each**, chosen in the Programs pilot away from the
  three port colours and checked as the verdicts' colours were (colour-vision deficiency, contrast
  in both themes). Today `data/status.yml` gives them LARMORX's apricot (`eco: larmorx`).
- **The site's accent** (focus rings, sliders, the brand) stays LARMORX's apricot: it marks
  controls, never data.
- **What the rollout corrects.** Today every port's panel label is apricot, the replica's too. The
  charts of the ANTs pages, `resample-series`, `sdcflows-fieldmap` and `sdcflows-estimation` mark
  their replicas as the apricot (`"mark": "larmorx"`), and so do 13 replica series of 3dvolreg's.
  `nifti-io`'s apricot stays, with the label "LARMORX's own". The evidence module names the port
  (`replica` or `clean-room`), the page's script colours it, and the images stay as they are.
- **The band of the original's own variability** is drawn today in the replica's blue (`band:
  P.replica`). Behind the replica's blue line it reads as the replica's range. The band belongs to
  the original, so it becomes a light wash of the original's ink.

## 6. The site's structure

| Page | What it holds | From today's site |
|---|---|---|
| Home | what LARMORX gives; original, replica and clean room with one example; the programs by software; how to use it; where it stands; the name and FAIR, in full | `index.md`, every part kept (the table below) |
| Getting started | installing, first steps, a short list of the packages (in full on Concepts), requirements, standalone programs, containers, checking a download, updating | `installation.md` |
| Concepts | the four words in depth; the packages and their licences, in full; `auto`, `fix` and `reproduce`; threads; why clean rooms exist and how the clean-room audit proves them clean; Why Rust and Python, its own page under it | parts of `index.md` and `validation.md`; `why-rust-and-python.md` |
| Programs | the programs grouped by software, each group opening (a toggle) to its programs: what each does, its ports, its status, links to its two pages | `tools/index.md`, `data/status.yml` |
| A program's reference | what it does; its command line with the original's arguments; its Python call; an options table generated from the code; examples the tests run; its ports; a link to its evidence | new, generated from `docs/api/` |
| A program's evidence | what is compared; the verdicts; the worked examples; the original's own variability; random testing; every case; speed and memory; corrections by default; known limits; History; credits | `tools/*.md`, every part kept |
| Validation | how LARMORX tests; how to read the evidence; what each verdict means; where every program stands; random testing, up to date; the records; the data panel, its own page under it | `validation.md`, `tools/index.md`'s "How to read the figures", `data-panel.md` |
| Roadmap | what is done, under way and next | `roadmap.md` |
| About | the licence, citing LARMORX, the two repositories, credits and notices; a link to the name and FAIR on Home | the footer |

The header's links: Getting started, Concepts, Programs, Validation, Roadmap, About.

### Addresses

| Page | Address | Today's address, which sends the reader on |
|---|---|---|
| Home | `/` | |
| Getting started | `/getting-started/` | `/installation/` |
| Concepts | `/concepts/` | |
| Why Rust and Python | `/concepts/why-rust-and-python/` | `/why-rust-and-python/` |
| Programs | `/programs/` | `/tools/` |
| A program's reference | `/programs/<group>/<program>/` (`/programs/fsl/bet/`, `/programs/ants/antsRegistration/`) | |
| A module's reference | `/programs/<group>/<module>/` (`/programs/fmriprep/resample-series/`) | |
| A program's evidence | `/evidence/<page>/` (`/evidence/fsl-bet/`) | `/tools/<page>/` |
| Validation | `/validation/` | |
| Data panel | `/validation/data-panel/` | `/data-panel/` |
| Roadmap | `/roadmap/` | |
| About | `/about/` | |

- **Groups**, in this order: `ants` (ANTs and ITK), `afni`, `fsl`, `tedana`, `sdcflows`, `fmriprep`,
  `larmorx` (LARMORX's own); `freesurfer` and `workbench` as planned. A group is the command's
  family for every command but `mri`.
- **A program's address** uses the command's own spelling (`antsRegistration`, `3dTshift`), so
  the address shows what the reader types. Pages link through the builder's `link()`, and the
  build reports a link to a page that does not exist, so a wrong letter case is caught.
- **A module's address** is its API page's name without the group's prefix:
  `/programs/ants/transforms-hdf5/`, `/programs/sdcflows/fieldmap/`,
  `/programs/sdcflows/niworkflows-helpers/`, `/programs/sdcflows/bids/`,
  `/programs/fmriprep/resample-series/`, `/programs/fmriprep/fieldmap-steps/`,
  `/programs/fmriprep/confounds/`, `/programs/fmriprep/ndimage-filters/`,
  `/programs/fmriprep/skimage/`, `/programs/larmorx/io/`.
- **The count**: 46 commands today (ANTs 13, AFNI 4, FSL 26, `mri` 1, tedana 1, sdcflows 1)
  give 45 reference pages, since `larmorx mri hmc` is on mcflirt's page; with the 10 modules, 55
  reference pages. 25 evidence pages, one per page of today's `content/tools/` (its index aside):
  `bet`'s covers `bet`, `bet2`, `betsurf`, `standard_space_roi` and `fslroi` and shows its three
  records (`fsl-bet`, `fsl-bet-replica`, `fsl-bet-replica-full`); each reference page links to its
  own section.
- **mcflirt's page** is `/programs/fsl/mcflirt/`. It shows `larmorx mri hmc` and `lx.mri.hmc` as
  its clean room, and `larmorx --implementation clean-room fsl mcflirt`, which runs it (from the
  next release; in 0.1.0rc1, the environment variable `LARMORX_IMPLEMENTATION=original`).
- **The API pages, by group**:
  - `ants`: the thirteen `ants-*` pages and `transforms-hdf5`;
  - `afni`: the four `afni-*` pages;
  - `fsl`: the eight `fsl-*` pages and `mri-hmc`;
  - `tedana`: `tedana-t2smap`;
  - `sdcflows`: `sdcflows-fieldmap`, `niworkflows-helpers`, `bids`;
  - `fmriprep`: `resample-series`, `fmriprep-fieldmap-steps`, `confounds`, `ndimage-filters`,
    `skimage`;
  - `larmorx`: `io`;
  - not a program: `upstream-defects`, which Validation links, as the home page links the register
    today.
- **One source.** `docs/api/<page>.md` stays the one source of a reference page. An exporter in
  this repository, beside the evidence module (step 2 of the plan), writes the site's reference
  pages from it, the command-line definitions and the `.pyi` stubs. Nobody edits a reference page
  on the site by hand. The exporter reads only files a clean-room implementer may read: the API
  page, the main package's Python (`python/larmorx`), the command-line code of the clean rooms
  and of `larmorx-cli`, larmorx-core's `cli.rs` and `defects.rs`, and `validation/tools.toml`;
  never the replica crates. So a reference page holds no fact from the restricted material
  (section 1).
- **An API page with several programs** holds one section per program, and the exporter writes
  one reference page from each section: `fsl-flirt.md` gives 14, `fsl-bet.md` 3, `fsl-roi.md`
  and `fsl-topup.md` 2 each. `sdcflows-fieldmap.md` gives the `sdcflows` command's page and the
  `fieldmap` module's.
- **Old addresses keep working.** The builder gets a redirect page type (step 2): a page at the
  old address that names the new one. It holds a table of the old page's anchors and the page
  that now holds each: `/tools/fsl-bet/#speed` goes to `/evidence/fsl-bet/#speed`, and
  `/tools/#reading` (how to read the figures) to `/validation/#reading`. A line of script
  sends the reader there with the anchor, which a plain meta refresh would drop, and a link to the
  new address serves readers without scripts. Every anchor of today's pages keeps working: the
  heading that takes a section's place gets its old `id` (Home's `#status`, Getting started's
  `#first-steps` and `#containers`).
- **Another letter case.** An address typed in another letter case
  (`/programs/ants/antsregistration/`) is not a page, and the build's link check sees only the
  site's own links. So the 404 page holds the list of the site's addresses, which the builder
  writes, and sends such a reader to the address that matches in lower case, anchor kept. No
  page is written twice in two letter cases. On the usual file systems of macOS and Windows,
  `antsRegistration/` and `antsregistration/` are one folder, so a site built there would let
  one page overwrite the other.

### Where every part of today's site goes

| Today | Its new place |
|---|---|
| Home: the hero | Home 1; its closing text rewritten (example 1), the animation's description in the new words |
| Home: the lead | Home 2 (example 2) |
| Home: "What LARMORX is" (`#what-it-is`): the lede, the Status note, the three cards, the software list | the lede and the cards to Home 3 (examples 3 to 6); the Status note to Home 2; the software list to Home 4, its full text to Programs |
| Home: "Core features" (`#features`): the six cards, the examples, the note below them | Home 5: the command line, Python, platforms and Validation cards; the Rust card in one line with a link to Why Rust and Python; the Licences card into Home 3's cards and Concepts; the examples and the note as they are |
| Home: "Philosophy" (`#philosophy`) | Home 7, in full: the lede (example 7), the equation, the text (example 8), the FAIR list, FAIR4RS |
| Home: "Current status" (`#status`): the lede, the "In the repository" note, the two tables, the install note | Home 6, all of it |
| Home: the closing link | Home 8 |
| Installation, every section | Getting started, each with its anchor; "The three packages and their licences" (`#packages`) a short list there, in full on Concepts |
| Why Rust and Python | its own page under Concepts |
| Validation: The method, Levels of testing, Now and before the first release, How to read the evidence, The verdicts, Where every tool stands, Random testing, The records | Validation, each with its anchor; "Three ways of reimplementing" becomes "Two kinds of port" (example 11), with a link to Concepts |
| Data panel | its own page under Validation |
| Tools: the introduction; "How to read the figures" (`/tools/#reading`) | Programs (example 9); Validation's "How to read the evidence" |
| A tool's page: "What it is", "What they are" (`ants-programs`, `ants-ai`) | the evidence page's first section, "What is compared", with its old anchor; the reference page's "what it does" is the API page's own text (One source, above) |
| A tool's page: "FLIRT's other programs" (`fsl-flirt`) | the reference pages of those programs (`avscale`, `img2imgcoord`, `rmsdiff`, `makerot`, `midtrans`, `pointflirt`, `applyxfm4D`, `aff2rigid` and the rest), each with its own section of `fsl-flirt.md` |
| A tool's page: the verdicts, the worked examples, the variations, every case, random testing, speed, "Corrections by default", "Known limits" | the evidence page, each with its anchor |
| A tool's page, every other section: "Memory", the original's own variability ("Against ANTs' own variability", "tedana's own variability"), "The originals' own step: two library releases", "On the data panel", "What is open", "Every file" and the rest | the evidence page, each with its anchor |
| A tool's page: "The standard images the variations read" (`#data`) | the evidence page, with its anchor; `bet`'s reference page links to it (its facts come from restricted material: section 1) |
| Roadmap | as it is |
| 404 | as it is, with the list of addresses for another letter case (Addresses, above) |

### The home page, section by section

1. **The hero**, as now: the animation and its still image, the name and the tagline (question 1
   below), one line on what LARMORX is, `pip install larmorx`, the platforms. Its closing text
   says what the animation shows (example 1).
2. **What LARMORX gives**, one short paragraph (example 2), and the state of the project: early
   development; 0.1.0rc1 is a pre-release.
3. **Original, replica, clean room.** Three short cards, one per word (examples 3 to 6), with the
   packages and their licences. One example runs through them. `bet` from FSL 6.0.7.23 is the
   original. Its replica is in `larmorx-nc`, its clean room in `larmorx`, written without FSL's
   source. On fMRIPrep's own call both give FSL's brain and mask, every voxel. Links go to
   Concepts and to `bet`'s evidence.
4. **Programs by their software**: each project, its colour, which ports exist and where; a link
   to Programs.
5. **How to use it**: the original's command line and the Python call, with the examples the tests
   run; LARMORX's three options (threads, port, defects), from the next release, with 0.1.0rc1's
   environment variables beside them; the same results on six platforms; defects corrected by
   default, with `reproduce` to get the original's.
6. **Where it stands**: the generated status table, the planned steps, the packages to install,
   the links to the API pages, the benchmark reports and the register.
7. **The name**, in full: the Larmor frequency (example 7) and FAIR (example 8), with the FAIR
   list and FAIR4RS.
8. **A closing link** to Validation and Getting started.

## 7. The old words today

From [the inventory](../validation/investigations/docs-revision/inventory.md) (this repository
at `6eb5f150`, the site at `d9cf5fa9`, with `bet`'s first reference and evidence pages;
`old_words.py` makes it again):

| Who changes them | Files to change | *clean-room original* | *original* for a clean room | *exact replica* | *port* as a kind | slogan | tagline | jargon on user pages | other *original* to read |
|---|---|---|---|---|---|---|---|---|---|
| the writer, in the rollout | 365 | 2,015 | 197 | 228 | 6 | 14 | 7 | 1,550 | 4,793 |
| generated: re-made or re-exported, never by hand | 64 | 1,482 | 156 | 19 | 0 | 0 | 0 | 2,612 | 2,692 |
| a clean-room agent, a spec writer, the orchestrator | 42 | 109 | 14 | 18 | 0 | 0 | 2 | 0 | 373 |
| kept as written (history, identifiers) | 377 | 1,944 | 3,769 | 149 | 5 | 6 | 2 | 8 | 11,987 |

The generated row holds the exported evidence (28 files to change), which the rollout exports
again. It also holds the records' and reports' data (4) and the pages made by a measurement
(section 1, "Where the old words stay"), those that hold old words: 21 records' pages, 10 of the
21 benchmark reports and the clean-room audit's page. These keep their words until each is made
again. On the site, the pages (`content/`) hold 259 *clean-room original*, 60 *exact replica*
and 294 words of jargon. The exported evidence holds 1,376 *clean-room original* and 143
*original* for a clean room. Of the 47 files named with *original*, 28 are renamed in the
rollout and 5 later, when the reports that link to them are made again (section 1, "Renamed
files"). 14 keep their names: 12 of random testing's cases and 2 investigation scripts. The last
column counts words to read one by one; most of them name the program compared with, which is
right.

## 8. Questions for the user

1. **The tagline** ("Bringing Neuroimaging Tools into Resonance. Reimplemented in Rust.
   Accessible through Python. Built for FAIR research.", yours, 2026-10-19). Recommendation: keep
   it in the hero, where the animation shows it; "Many toolkits, one framework" goes.
2. **Colours shared by a port and something else** (section 5). Three verdicts and the difference
   maps share the apricot and the blue with the ports; ANTs' and LARMORX's own colours are the
   replica's and the clean room's. Recommendation: keep the verdicts' and the difference maps'
   colours, with the rule that ports are lines and labels and verdicts filled areas with a
   pattern (changing them would redraw 81 images); show the software colours only as pastels
   with their names, and give tedana, sdcflows and fMRIPrep a pastel each.
3. **Shorter pages.** The rewrite shortens the prose, so a page's word count falls; every number,
   figure, table, viewer, image, link and fact stays. Recommendation: the merge's before/after
   inventory compares everything except words and lists the word counts beside it; the reviewer
   checks page by page that every fact stays.
4. **Which version the pages describe.** Recommendation: `main`, which the tests run, with what
   0.1.0rc1 lacks marked "from the next release" and 0.1.0rc1's way beside it (section 2, rule
   14). The other way, describing 0.1.0rc1, would leave tedana's clean room and the three options
   off the pages until the next release.
5. **Developer words.** Developer documents keep *oracle*, *upstream*, *parity* and *the main
   package*, since directories and modules carry these names (`oracles/`, the oracle-output
   cache, `tests/parity`); user pages use the new words. Recommendation: yes.
