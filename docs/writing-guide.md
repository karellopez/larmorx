# Writing guide

How LARMORX's documentation is written: the website, this repository's documents and API pages,
and the lines its programs print. The documentation revision
([documentation-plan.md](documentation-plan.md)) rewrites both repositories with it, and every
page written afterwards follows it. Draft of 2026-11-12, for the user's approval.

## 1. Four words

| Word | What it means | Example |
|---|---|---|
| **original** | The program LARMORX is compared with, as its own developers release it. | `bet` from FSL 6.0.7.23; `antsRegistration` from ANTs 2.6.5 |
| **port** | Any program LARMORX reimplements in Rust. Every port is a replica or a clean room. | `bet` has two ports; `antsRegistration` has one. |
| **replica** | A port translated from the original's source, step by step, to give the original's results bit for bit. Where a record shows a difference, it says why (most often the C library's rounding of `exp` or `pow`). A replica keeps the original's licence. | `antsRegistration`'s replica in `larmorx`; `bet`'s in `larmorx-nc` |
| **clean room** | A port written again without the original's source or its replica: from a description of what the original does, its documentation, papers and runs of the original. It is built for speed and carries LARMORX's licence, Apache-2.0. | `bet`'s and `t2smap`'s clean rooms in `larmorx` |

Where each port lives today:

| Software | Replica | Clean room |
|---|---|---|
| ANTs and ITK | `larmorx` | planned, after FreeSurfer |
| AFNI | `larmorx` (with AFNI's CC BY 4.0 credit) | `larmorx` |
| FSL | `larmorx-nc` (non-commercial use only) | `larmorx` |
| tedana (`t2smap`) | `larmorx-gpl` (GPL-3.0) | `larmorx` |
| sdcflows, fMRIPrep's steps and the Python libraries they call | `larmorx` | planned, after FreeSurfer |

How to use them:

- **original** never names a LARMORX program. Give the version where it matters: "FSL 6.0.7.23".
- **clean room** is a noun of two words: "the clean room", "`bet`'s clean room", "clean rooms".
  Before another noun it takes a hyphen: "clean-room code", "the clean-room audit". Never
  "clean-room original", "clean-room implementation" or "clean-room port".
- **replica** stands alone. Not "exact replica" or "bit-exact replica": the word already says it.
  Where the reader needs the fact, give it with its number: "the replica gives FSL's files byte
  for byte in all 201 compared cases".
- **port** covers both kinds. Never use it for one kind: ANTs' ports are replicas, not "ported
  from the source" as if that were a third way.
- In code the names are `implementation="auto" | "replica" | "clean-room"` and
  `--implementation clean-room`. The published release, 0.1.0rc1, spells the clean room
  `"original"`, and `main` still accepts that spelling. Until the next release, a page that shows
  the value to a user gives `"original"` for 0.1.0rc1 and names `"clean-room"` beside it.
- What LARMORX has that is not a port is **LARMORX's own**: its NIfTI reader and writer (compared
  with nibabel), the `larmorx` command itself.

### Other words, one meaning each

| Write | Not | Note |
|---|---|---|
| the original; a run of the original | the oracle; the upstream | Pages users read. Developer documents keep "oracle" and "upstream", and so do names (the oracle-output cache). |
| the comparison with the original; the record | parity; the parity tests | Same note. |
| the quick check, the standard comparison, the full comparison | the smoke, standard and full tiers | Say what runs and when; the names *smoke*, *standard* and *full* once, in italics, after the explanation. |
| the cross-platform checks | the golden tests | On user pages. |
| a random-testing run of half an hour | a trial night; a night | Give the duration. |
| a defect of the original, corrected by default | D13 | The register's ids (FSL-11, AFNI-41) stay, linked to the register. |
| `fix` (the default) and `reproduce` | default mode; reproduce mode | The values of `upstream_defects` and `--upstream-defects`. |
| the software (ANTs, FSL) | the ecosystem; the toolkit | In prose. |
| the family (`fsl` in `larmorx fsl bet`) | | Only for the command line's first word after the options. |
| program; function (a Python call with no command line) | tool | "tool" only in names that exist (`validation/tools.toml`). |
| port; the replica; the clean room | implementation | `implementation` stays the option's name. |
| `larmorx` (the package); LARMORX (the project) | the main package; LARMORX itself | Developer documents keep "the main package". |
| `larmorx-nc`, `larmorx-gpl` | the companion wheels | Say what each holds and its licence. |

### Where the old words stay

- **History**, as written: `docs/parts/`, the decision log's entries before 2026-11-12,
  `docs/status-log.md`, the released sections of `CHANGELOG.md`, commit messages, and
  `validation/investigations/` and `validation/probes/` (the tools and outputs behind past
  findings).
- **Identifiers**: random testing's case names, and the record, suite and cache names written
  inside records until a record is made again. Code keeps accepting `"original"`.
- **Who changes the rest**: the writer changes the site, the documents, the API pages, the
  Python package's messages and docstrings and the evidence module's texts. A clean-room agent
  changes the clean rooms' code and the lines they print (`crates/larmorx-afni`, `-mri`, `-fsl`,
  `-tedana` and larmorx-core's clean-room files), from the writer's list. A spec writer changes
  `specs/`. The orchestrator changes `CLAUDE.md`.
- **Renamed files**: records, findings, benchmark reports, tests and validation modules named
  `*-original*` become `*-clean-room*` (`*_clean_room*` for Python modules), with their links and
  the generated tables. Random testing's cases and the investigations keep their names.

[validation/investigations/docs-revision/inventory.md](../validation/investigations/docs-revision/inventory.md)
counts every old word, file by file (section 7).

## 2. The tone

1. **Write as a person tells a colleague.** Plain words, the program as the subject, active
   voice: "`bet` finds the brain in a head image", not "brain extraction is performed".
2. **One idea per sentence.** Most sentences under 25 words. A semicolon or a bracket only where
   it helps, never one bracket inside another.
3. **What the reader gets comes first; history comes last.** A lede says, in at most two
   sentences, what the program does and how good each port is today. The dates of changes go to
   the page's History section.
4. **Say what a thing is.** "The standard comparison: every option and fMRIPrep's own calls",
   not "the standard tier".
5. **Numbers with their unit and their whole.** "198 of 200 compared cases", "3.0 s", "302 MB",
   "10 to 15 times faster on one thread". Every number comes from a record, linked once.
6. **No slogans, no filler, no praise.** Leave out *established*, *aims to*, *robust*,
   *seamless*, *powerful*, *state-of-the-art*, *comprehensive*, *framework*, *toolkit*,
   *leverage*, *simply*, *just*, *note that*, *in order to*. A phrase repeated on every page
   ("every option, message, exit status and failure case") is said once, on the Validation page,
   and linked.
7. **Claim what the record shows, no more.** "Gives FSL's files byte for byte in all 201
   compared cases", not "is identical to FSL".
8. **British English** (behaviour, colour, licence as a noun and license as a verb), as the site's
   `en-GB`. Dates as 2026-11-12.
9. **Names.** LARMORX in prose and `larmorx` in commands; software in plain text (FSL, AFNI);
   programs, options, files and values in code (`bet`, `-f 0.2`, `"clean-room"`). A version goes
   with its software: "`bet` from FSL 6.0.7.23" in prose, "FSL `bet` 6.0.7.23" in titles and
   labels.
10. **Licences in plain words.** "Non-commercial use only"; "whoever passes it on must pass on its
    source under the same licence".
11. **Copyable shell blocks hold commands only**: no comments (zsh, macOS's shell, reads `#` as
    an argument), no prompts, every line runnable as it stands. The tests run every example.
12. **Link instead of repeating**, and explain a term at its first use on a page or link Concepts.

## 3. Before and after

Real sentences of today's site and documents, rewritten. Numbers in the rewrites come from the
same records as the originals.

**The home page** (`content/index.md` of the site)

1. Before (the end of the hero): "Many toolkits, *one* framework. Spins of five software
   ecosystems, precessing at one Larmor frequency, gather into one brain. Each keeps its colour."
   After: "Spins in resonance. Five groups of spins, one colour for each software LARMORX draws
   from (ANTs, AFNI, FSL, FreeSurfer and Workbench), come to one frequency and gather into a brain
   from the MNI template."
   Why: the slogan goes; the text says what the animation shows.
2. Before: "LARMORX is an open-source, cross-platform neuroimaging toolbox that reimplements
   established neuroimaging algorithms and command-line tools in Rust. It aims to reproduce the
   functionality and behaviour of tools from ANTs, AFNI, FreeSurfer and others, without requiring
   you to install the original binaries or manage complex system dependencies."
   After: "LARMORX puts programs from ANTs, AFNI, FSL, tedana, sdcflows and fMRIPrep in one Python
   package. `pip install larmorx` is all it takes on Linux, macOS and Windows: no other
   installation, and the same results on every computer (the tests check it on six platforms)."
   Why: what the reader gets, in their words; no *established* or *aims to*; FreeSurfer is not
   ported yet, so it is not named.
3. Before: "Each tool is reimplemented in one of three ways, chosen by the licence of the
   original. Where a licence requires it, algorithms are reimplemented under clean-room
   development practices, so that restrictively licensed source code is not incorporated."
   After: "LARMORX's programs are ports of original programs. Each port is a replica or a clean
   room, and some programs have both. The original's licence decides where each can live."
   Why: two kinds of port, not three ways; the set phrase about practices goes.
4. Before: "Ported from the source. Tools with permissive licences, ANTs and ITK (Apache-2.0),
   are ported from their source, with each Rust function's origin recorded in the crate's
   `PROVENANCE.md`."
   After: "Replica. The original's source translated into Rust, step by step, to give its results
   bit for bit. A replica keeps the original's licence: ANTs', ITK's and AFNI's replicas are in
   `larmorx`, FSL's in `larmorx-nc` and tedana's `t2smap` in `larmorx-gpl`."
   Why: ANTs' ports are replicas.
5. Before: "A clean-room original. Where the licence is restrictive (FSL, tedana, Workbench), an
   original is written from a behavioural specification, documentation and runs of the original
   program, never from its source."
   After: "Clean room. Written again from a description of what the original does and from runs of
   it, never from its source, and built for speed. It carries LARMORX's licence, Apache-2.0, so it
   is in `larmorx`: AFNI's, FSL's and tedana's programs have one."
   Why: *original* named two things in one sentence; Workbench has no port yet.
6. Before: "Tools with both take `implementation="auto" | "replica" | "original"`."
   After: "A program with both ports takes `implementation="replica"` or `"clean-room"`
   (`"original"` in 0.1.0rc1). `"auto"`, the default, runs the replica when its package is
   installed."
   Why: the value is `"clean-room"` since the easy command line; 0.1.0rc1's users still need
   theirs; and what `auto` does.
7. Before: "In magnetic resonance, spins of one species share one frequency. LARMORX would like the
   same for neuroimaging tools: many ecosystems, one shared objective."
   After: "The name comes from the Larmor frequency: in a magnetic field, spins of one kind precess
   at one frequency. LARMORX brings programs from different software into one package and
   compares each with its original the same way."
   Why: the metaphor's slogan goes; the sentence says what is shared.

**Other pages of the site**

8. Before (`content/tools/index.md`): "Each page says what the tool is, how LARMORX implements it
   (a port of the source, a clean-room original, an exact replica) and how to call it, then shows
   its validation record in figures."
   After: "Each program has two pages: its reference (what it does, its command line, its Python
   call, its options and examples) and its evidence (how its ports compare with the original)."
9. Before (`content/validation.md`): "**The original program is the oracle.** Each LARMORX tool is
   run beside the program it reimplements, on the same files with the same arguments, and their
   outputs, exit statuses and messages are compared."
   After: "**Every port runs beside its original**, on the same files with the same arguments.
   Their outputs, exit statuses and messages are then compared."
10. Before (`content/validation.md`): "**Three ways of reimplementing.** A *port* translates source
    whose licence allows it (ANTs, ITK) and aims at the original's bits. Where the licence
    restricts the source (FSL, Workbench), there are two: an exact *replica* in a separate package
    under the original's licence (`larmorx-nc` for FSL), which aims at the original's bits, and a
    *clean-room original* in LARMORX itself (Apache-2.0) ..."
    After: "**Two kinds of port.** A replica is translated from the original's source and aims at
    its exact bits. A clean room is written without the source and aims at the original's
    behaviour, within stated thresholds, and at speed. ANTs' ports are replicas; AFNI's, FSL's and
    tedana's programs have both."
11. Before (`content/validation.md`): "**Parity tiers.** Each tool's suite of cases runs in tiers:
    *smoke* under pytest, *standard* for the records quoted on this site (...), *full* where a tool
    has more."
    After: "**Three sizes of comparison.** A quick check of a few cases of every program at every
    merge (*smoke*); the standard comparison, every option and fMRIPrep's own calls, which this
    site shows (*standard*); and for some programs more and larger cases (*full*)."
12. Before (`content/tools/fsl-bet.md`, the lede, 137 words): "FSL's brain extraction (bet, bet2)
    and its skull and scalp surfaces (betsurf), in two implementations: a clean-room original in
    LARMORX (Apache-2.0), experimental, written from a behavioural specification without FSL's
    source, whose bet2 and bet give FSL 6.0.7.23's files bit for bit in nearly every case ... and
    since 2026-11-05 so does the clean-room original, every program its own clean-room program."
    After: "`bet` finds the brain in a head image; `betsurf` adds the skull and the scalp. The
    replica (`larmorx-nc`) gives FSL 6.0.7.23's files byte for byte in all 201 compared cases; the
    clean room (`larmorx`) gives FSL's values bit for bit in 378 of the 380 images its record
    compares."
    Why: what it does and how good each port is; the dates go to the page's History.
13. Before (`content/tools/fsl-bet.md`, the facts): "Status: experimental (original) · validated
    (replica)".
    After: "Status: replica validated; clean room experimental."
14. Before (`content/tools/fsl-bet.md`): "The original's record runs the replica's standard cases
    against FSL 6.0.7.23, betsurf's real heads and, since 2026-11-05, the variations,
    standard_space_roi and fslroi (the record, 252 cases): 198 of its 200 compared cases are within
    the thresholds, 46 more rejected by both alike and 6 where FSL crashes and `reproduce` fills
    what FSL leaves undefined."
    After: "The clean room's record compares it with FSL 6.0.7.23 in 252 cases: the replica's
    cases, real heads for `betsurf`, and `bet`'s options with `standard_space_roi` and `fslroi`.
    198 of the 200 compared cases are within the thresholds. 46 more are rejected by both (FSL
    refuses or crashes, and the clean room refuses). In 6, FSL crashes, and `reproduce` fills in
    what FSL leaves undefined."
15. Before (`site.yml`, N4's summary): "Bias-field correction, as fMRIPrep, sMRIPrep and sdcflows
    call it. Port."
    After: "Bias-field correction, as fMRIPrep, sMRIPrep and sdcflows call it. Replica."
16. Before (`content/roadmap.md`): "**Clean-room originals of the permissive ports** | A clean-room
    original, built for speed, of every tool ported from permissively licensed code ..."
    After: "**Clean rooms for the programs that have only a replica** | A clean room, built for
    speed, of every program that has only a replica: ANTs and ITK, sdcflows' field maps,
    fMRIPrep's resampler and Python steps, nitransforms, the confounds."
17. Before (`data/verdicts.yml`, a verdict's name): "Deliberate correction (D13)".
    After: "Deliberate correction". Why: D13 is a decision's number; the Validation page says what
    the verdict means and links the register.

**The evidence** (`static/evidence/fsl-bet.json`, written by the evidence module)

18. Before (a difference panel): "original − FSL 6.0.7.23", above it "larmorx against the
    original".
    After: "clean room − FSL `bet` 6.0.7.23", above it "a port against the original".
    Why: in the old caption *original* named LARMORX's program in one line and FSL's in the next.
19. Before (an image panel): "larmorx's clean-room original", below it "default mode (Apache-2.0,
    crate larmorx-fsl), against FSL 6.0.7.23".
    After: "clean room", below it "LARMORX's `bet`, `fix` (the default)".
20. Before (the speed chart): "Seconds for the whole command (read, compute, write), median of
    repeated runs, log scale; the factor is the clean-room original's speed-up over FSL at one
    thread".
    After: title "Speed, both ports against the original: FSL `bet` 6.0.7.23"; caption "Seconds for
    the whole command (reading, computing, writing), the median of repeated runs, on a log scale.
    The factor is how many times faster the clean room is than FSL, both on one thread."
21. Before (an agreement chart): "larmorx clean-room original, against FSL 6.0.7.23 (standard
    tier)".
    After: "Agreement, clean room against the original: FSL `bet` 6.0.7.23 (standard comparison,
    252 cases)".
22. Before (a worked example): the title "fMRIPrep's skull strip of a real BOLD reference" and no
    summary line.
    After: the same title, then "What this shows: on fMRIPrep's own call, the replica and the clean
    room both give FSL's brain and mask, every voxel." with the verdict *bit-identical*.

**This repository**

23. Before (`README.md`): "Where a licence requires it, algorithms are reimplemented under
    clean-room development practices, so that restrictively licensed source code is not
    incorporated."
    After: "Where the original's licence keeps its code out of an Apache-2.0 package, LARMORX
    writes a clean room: a new program built from a description of what the original does, never
    from its source."
24. Before (`README.md`, the packages): "`larmorx` | every tool below: the ports, the clean-room
    originals and the AFNI replicas".
    After: "`larmorx` | the replicas (ANTs and ITK, AFNI, sdcflows, fMRIPrep's steps and the Python
    libraries they call), the clean rooms (AFNI, FSL, tedana's `t2smap`) and LARMORX's own NIfTI
    reader and writer".
25. Before (`docs/api/fsl-bet.md`): "**Two implementations.** ... the bit-exact **replica**,
    `larmorx-nc fsl bet|bet2|betsurf` ..., and the clean-room **original** in the main package
    (Apache-2.0, `crates/larmorx-fsl`), written from `specs/bet.md`, the BET paper and black-box
    runs of FSL".
    After: "**Two ports.** The replica, in `larmorx-nc` (FSL's licence: non-commercial use only),
    translated from FSL's source; and the clean room, in `larmorx` (Apache-2.0), written from
    `specs/bet.md`, the BET paper and runs of FSL."
26. Before (`docs/overview.md`, the words): "**Upstream** | An original program we reproduce";
    "**Clean-room original** | Our own code, written by someone who never sees the upstream source
    (or its replica)."
    After: "**Original** | The program a port is compared with, as its developers release it";
    "**Clean room** | A port written by someone who has never seen the original's source or its
    replica."
27. Before (a line `larmorx afni 3dvolreg -help` prints, from a clean room's code): "Usage: 3dvolreg
    [options] dataset   (larmorx: clean-room original of AFNI 26.3.05's 3dvolreg)".
    After: "Usage: 3dvolreg [options] dataset   (larmorx: the clean room of AFNI 26.3.05's
    3dvolreg)". Who: a clean-room agent, from the writer's list.

## 4. Figures and their titles

**The pattern:** *what it shows*, *A* against *B*: *software* `program` *version*.

- *A* and *B* are the four words: the replica, the clean room, both ports, the original. A
  comparison of the original with itself (thread counts, builds, versions) says so: "the original
  against itself".
- The title names the version of the original compared with. The caption below says what the
  axes, colours and units are, and on what data.

| Figure | Title |
|---|---|
| agreement chart | Agreement, clean room against the original: FSL `bet` 6.0.7.23 |
| speed chart | Speed, both ports against the original: FSL `bet` 6.0.7.23 |
| the original's own spread | Variability, the original against itself: FSL `bet` 6.0.7.17 and 6.0.7.23 |
| random testing | Random testing, both ports against the original: FSL `bet` 6.0.7.23 |
| case table | Every case, replica against the original: FSL `bet` 6.0.7.23 |
| difference panel | clean room − FSL `bet` 6.0.7.23 (always *A* − *B*, LARMORX's port first) |
| image panel | its name ("FSL `bet` 6.0.7.23", "replica", "clean room"); a second line for how it ran (`fix` or `reproduce`, threads, build) |

**A worked example** has a title that names the data and the call ("fMRIPrep's skull strip of a
BOLD reference"), then one line, "What this shows: ...", with the verdict of the comparison that
matters most. Its panels go in one order: the input, the original, the replica, the clean room.
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
image has to be drawn again. A point is filled at one thread and open at more, as the speed charts' dots
and rings are now. Where the three lines coincide, as they do when the ports are bit-identical,
the dashes let the original's line show through.

- **A port's colour always comes with its name** (a legend, a caption) and its mark, never alone.
- **Ports are lines, marks and labels; verdicts are filled areas.** A verdict's colour always comes
  with its pattern and its name (`data/verdicts.yml`). Two verdicts share a hue with a port
  (*bit-identical* the apricot, *within the original's variability* the blue); a chart coloured by
  verdict names the port in its title.
- **The difference maps keep their scale**: blue below zero, apricot above, with the colour bar
  that says so. 81 posters are drawn with it, and redrawing them is not part of the revision.
- **The software pastels** (`data/ecosystems.yml`) mark the software groups on the Programs page
  and the dots beside program names, never a port. tedana, sdcflows and fMRIPrep, which have no
  pastel, take the ink.
- **What the rollout corrects:** today every port's panel label is drawn in the apricot, the
  replica's too; the charts of the ANTs pages and 13 replica series of 3dvolreg's page are marked
  as the apricot (`"mark": "larmorx"`); and the band of the original's own variability is drawn in
  the replica's blue, where a light wash of the original's ink belongs. The evidence module names
  the port (`replica` or `clean-room`) and the page's script colours it; the images stay as they
  are.

## 6. The site's structure

| Page | What it holds | From today's site |
|---|---|---|
| Home | what LARMORX gives; original, replica and clean room with one example; the programs by software; how to use it; where it stands; the name | `index.md`, every part kept |
| Getting started | installing, first steps, the packages and their licences, requirements, standalone programs, containers, checking a download, updating | `installation.md` |
| Concepts | the four words in depth; the packages and licences; `auto`, `fix` and `reproduce`; threads; why clean rooms exist and how the clean-room audit proves them clean; why Rust and Python | parts of `index.md` and `validation.md`; `why-rust-and-python.md` under it |
| Programs | the programs grouped by software, each group opening (a toggle) to its programs: what each does, its ports, its status, links to its two pages | `tools/index.md`, `data/status.yml` |
| A program's reference | what it does; its command line with the original's arguments; its Python call; an options table generated from the code; examples the tests run; its ports; a link to its evidence | new, mostly from `docs/api/` |
| A program's evidence | what is compared; the verdicts; the worked examples; the original's own spread; random testing; every case; speed and memory; History; credits | `tools/*.md`, every part kept |
| Validation | how LARMORX tests; what each verdict means; where every program stands; random testing, up to date; the data panel | `validation.md`, `data-panel.md` |
| Roadmap | what is done, under way and next | `roadmap.md` |
| About | the name and FAIR, the licence, citing LARMORX, the two repositories, credits and notices | parts of `index.md` and the footer |

- **Groups on the Programs page:** ANTs and ITK, AFNI, FSL, tedana, sdcflows, fMRIPrep, LARMORX's
  own; FreeSurfer and Workbench as planned.
- **One reference page per program** (`bet`, `bet2` and `betsurf` each have one). **One evidence
  page per record**, as today: `bet`'s evidence covers `bet`, `bet2`, `betsurf`,
  `standard_space_roi` and `fslroi`, and each reference page links to its own section.
- **Addresses:** `/programs/<family>/<program>/` for a reference page (`/programs/fsl/bet/`) and
  `/evidence/<page>/` for an evidence page (`/evidence/fsl-bet/`). Each old address
  (`/tools/fsl-bet/`, `/installation/`) stays as a page that sends the reader on, so no link
  breaks.

### The home page, section by section

1. **The hero**, as now: the animation and its still image, the name and the tagline (question 1
   below), one line on what LARMORX is, `pip install larmorx`, the platforms. Its closing text
   says what the animation shows (example 1).
2. **What LARMORX gives**, one short paragraph (example 2) and the state of the project (early
   development; 0.1.0rc1 is a pre-release).
3. **Original, replica, clean room.** Three short cards, one per word (examples 3 to 6), with one
   example running through them: `bet` from FSL 6.0.7.23 is the original; its replica in
   `larmorx-nc` gives FSL's files byte for byte; its clean room in `larmorx`, written without
   FSL's source, gives the same brain and mask on fMRIPrep's own call. Links to Concepts and to
   `bet`'s evidence.
4. **Programs by their software**: each software, its colour, which ports exist and where; a link
   to Programs.
5. **How to use it**: the original's command line, the Python call, LARMORX's three options
   (threads, port, defects), the same results on six platforms, the defects corrected by default
   with `reproduce` to get the original's; the command-line and Python examples, run by the tests.
6. **Where it stands**: the generated status table, the planned steps, the packages to install.
7. **The name**: the Larmor frequency and FAIR (example 7), with the FAIR list.
8. **A closing link** to Validation and Getting started.

## 7. The old words today

From [the inventory](../validation/investigations/docs-revision/inventory.md) (this repository
at `2827ad12`, the site at `b768293a`; `old_words.py` makes it again):

| Who changes them | Files to change | *clean-room original* | *original* for a clean room | *exact replica* | jargon on user pages | other *original* to read |
|---|---|---|---|---|---|---|
| the writer, in the rollout | 393 | 2,080 | 206 | 246 | 1,526 | 5,376 |
| generated: re-made or re-exported | 32 | 1,400 | 143 | 1 | 2,612 | 2,047 |
| a clean-room agent, a spec writer, the orchestrator | 42 | 109 | 14 | 18 | 0 | 373 |
| kept as written (history, identifiers) | 377 | 1,944 | 3,769 | 149 | 8 | 11,987 |

On the site alone, the pages (`content/`) hold 243 *clean-room original*, 60 *exact replica* and
271 words of jargon, and the exported evidence 1,376 *clean-room original* and 143 *original* for
a clean room. 33 files are renamed; 14 keep their names (12 of random testing's cases, 2
investigation scripts). The last column counts words to read one by one: most of them name the
program compared with, which is right.

## 8. Questions for the user

1. **The tagline** ("Bringing Neuroimaging Tools into Resonance. Reimplemented in Rust.
   Accessible through Python. Built for FAIR research.", yours, 2026-10-19). Recommendation: keep
   it in the hero, where the animation shows it; "Many toolkits, one framework" goes.
2. **Colours shared by a port and a verdict** (section 5). Recommendation: keep the verdicts' and
   the difference maps' colours, with the rule that ports are lines and labels and verdicts filled
   areas with a pattern. Changing them would redraw 81 images.
3. **Shorter pages.** The rewrite shortens the prose, so a page's word count falls; every number,
   figure, table, viewer, image and link stays. Recommendation: the merge's before/after inventory
   compares everything except words, and lists the word counts beside it.
