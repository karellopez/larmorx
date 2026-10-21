# Figures and evidence: what the documentation site shows of each tool

Every tool's contract includes visual evidence (CLAUDE.md rule 7): how the port, the clean-room
original and the replica compare with the original program. Since 2026-10-21 (decision log,
"Validation evidence, interactive and explained") it is **interactive evidence**: for each tool,
worked examples on real data that a reader inspects in an image viewer (the input, the original
program's outputs, larmorx's outputs, the differences, each panel labelled with the exact
program, version, build, thread count and mode), the metrics of every comparison computed
offline in float64, a verdict per comparison and per parity case from one taxonomy with its
reason, and every parity case in a searchable table. N4BiasFieldCorrection and 3dTshift were the
pilot (part 2a); part 2b brought every other tool into the same form (2026-10-21), and the static
figures of part 1 left the site.

The code that computes everything lives here, in `larmorx_validation`, so the evidence can be
exported again from a commit of larmorx; the site's repository (`larmorx_documentation`) holds
the exported data and the components that display it, and computes nothing of its own. Neither
runs in CI (rule 10).

## Export

```bash
pip install -e "validation[figures]"   # matplotlib, fonttools, nibabel, scipy (and Pillow)
# larmorx itself: a release build of this checkout; the oracles named in CLAUDE.md
python -m larmorx_validation evidence ants-n4 --site ../larmorx_documentation
python -m larmorx_validation evidence all --site ../larmorx_documentation   # every tool + summary
```

| Argument | Effect |
|---|---|
| `<tool>`, `summary` or `all` | a tool of `larmorx_validation.evidence.TOOLS`; `summary` writes the Validation page's matrix of every tool; `all` does every tool and then the summary |
| `--site DIR` | the site's checkout: writes `static/evidence/<tool>.json`, `static/evidence/summary.json` and the images in `static/evidence/blobs/`, and removes the blobs no JSON refers to any more |
| `--cache DIR` | the figures cache (default: `LARMORX_FIGURES_CACHE`, else the workspace's `oracles/figures-cache`) |
| `--no-compute` | never compute a missing cache entry: skip the tools that need one |

The first export of a tool computes its cache entry (minutes: the original programs at the
thread counts and maths libraries the comparison needs, fMRIPrep's own tools in its image,
larmorx's implementations in both modes); later exports reuse it and take seconds. Then build
the site (`tools/build.py`, 0 problems) and run its number check (its README, "The validation
evidence").

## What a tool's evidence is made of

- **Cache entries** (`figures/cache.py`): `<cache>/<tool>/<case>/` holds the outputs of every run
  and `RUN_INFO.json`, written last: each command with its environment, threads, exit status
  and time, the program versions, the larmorx commit and the inputs' SHA-256. An entry without
  `RUN_INFO.json` is computed again. Never commit cache contents (image data).
- **A worked example** (`evidence/example.py`): the pipeline's own call where there is one (read
  from an fMRIPrep oracle run's working directory: `command.txt` and the node's input files,
  plus fMRIPrep's own output as a panel), run by the original program (as built, with CORE-MATH
  preloaded, at other thread counts, fMRIPrep's own build in its image) and by larmorx (each
  implementation, default and `reproduce` modes, 1 and 12 threads). `Example.add` stores each
  panel once; `Example.compare` computes A − B over every voxel of the files (largest |Δ| and
  where, values that differ and their share, RMS, |Δ| over B's range, 1 − NCC) and where the
  shown brain differs most. Each comparison's verdict is decided by code from these metrics
  (e.g. *identical with correctly rounded maths* only when larmorx − original-with-CORE-MATH is
  bit-identical), never typed; the chain of causes asserts every identity it shows.
- **The display data**: the file's voxels reoriented to RAS (flips and transposes only) and
  cropped to the brain (fMRIPrep's mask, dilated: no face can show), zero outside it; stored as
  raw little-endian values in byte planes, zlib-compressed, or as the exclusive or of their bits
  with a reference (another panel, or a float32 quotient such as N4's input ÷ bias field), or
  as `same`/`neg` of another panel, whichever is smallest: lossless, and float32 or int16 as the
  outputs are, so the viewer reads exact values. Blobs are named by their SHA-256 (byte-
  deterministic with this machine's zlib), so an unchanged result changes nothing in git. Still
  posters (lossless WebP) of the default view serve readers without JavaScript. Budget: about
  25 MB per page of examples in the site's repository (the pilot: 18 MB for both tools).
- **The cases** (`evidence/<tool>.py`, `_cases`): every case of the tool's records with its
  description and command line (from the parity suite's own definitions), outcome, checks and a
  verdict: the records' JSON keeps each case's *reported* values (`__main__._summary`: checks
  whose threshold is `reported` or `reported (...)`, e.g. how many values differ from the
  original's, or the comparisons glibc's maths decided) and the *measured* value of every other
  check (e.g. a run's 95th-percentile deviation), so a case's verdict is read, not inferred from
  totals. Where a record cannot settle a verdict (ImageMath's cases within a tolerance), the
  cases are run again into a cache entry (ANTs at several thread counts and with CORE-MATH) and
  classified from those runs.
- **Charts' data**: the agreement overview (cases by category and verdict), the differences
  against the original's thread band (from the record's table), speed (from the benchmark
  report, never from informal timings), dots on a log axis (`dots`, e.g. antsRegistration's
  thread band beside larmorx's distance), line charts (an example's `lines`, e.g. a run's motion
  parameters over its volumes with the original's perturbation band) and tables (`tables`:
  numbers with their formats, text, verdicts; e.g. a transform's parameters, every dataset of
  an `.h5` file, every NIfTI header field).
- **Shared pieces** (`evidence/common.py`): the oracles, ITK fields and linear transforms read
  as stored, the comparisons of an example with their metrics, the default view (the slice with
  the most brain, the crosshair inside it where two outputs differ most), windows and posters,
  tables, the speed chart from a benchmark report, a record case's outcome verdict. An example
  may show a subset of a 4D series' volumes (`grid.t_index` names the file's volumes) and
  float64 outputs (stored as their bits, as float32).
- **Provenance** inside each tool's JSON: the records (path, SHA-256, commit), the benchmark
  reports, the cache entries (commands, versions, threads), the data (dataset, subject, licence)
  and the exporter's git tree hash.

## The verdicts

One taxonomy for the whole site (`larmorx_validation.evidence.core.VERDICTS`; definitions,
labels and colours in the site's `data/verdicts.yml`): bit-identical; identical with correctly
rounded maths; within the original's own variability; deliberate correction (D13); expected
divergence; rejected by both; open difference (not accounted for by the others, or above the
thresholds: it can still be improved, and what is known is stated). Rows comparing the original
with itself (thread counts, builds, versions) carry no verdict: they are the yardstick.

## Add a tool

1. Create `validation/src/larmorx_validation/evidence/<tool>.py` with `export(ctx) -> dict`, and
   add it to `TOOLS` in `evidence/__init__.py`; take `ants_n4.py` (a port, one output pair, a
   thread band), `afni_tshift.py` (two implementations, 4D, a D13 correction),
   `ants_registration.py` (several outputs: an image and a displacement field's components;
   tables), `mri_hmc.py` (a clean-room original against a perturbation band; a subset of
   volumes; line charts), `ants_programs.py` (six programs on one page, one case explorer each)
   or `itk_hdf5.py` (no images: tables) as the model, and `common.py`'s pieces.
2. The cache entry: the pipeline's real call on real data where there is one (an fMRIPrep
   oracle run's node), the original as built, with CORE-MATH, at the thread counts its record's
   band uses, fMRIPrep's own build if it differs; larmorx in both modes and at 1 and 12 threads.
   Keep it to minutes.
3. The example: panels (`Example.add`; order them so the closest references come first), the
   comparisons (`_diffs`), their verdicts from the metrics, the chain of causes where larmorx
   and fMRIPrep differ, a default view worth seeing (a slice through tissue where the tool
   acts, the crosshair on a difference), the facts (data, node, command, programs, grid) and
   the provenance. A D13 correction gets an example of its own where a real input can carry the
   defect (as AFNI-01's re-stored run).
4. The cases: each case's verdict from the record (re-generate the record from a clean commit if
   it predates the *reported* values), its description and command line from the suite.
5. The summary: move the tool in `evidence/summary.py` from its part-1 figure to its evidence.
6. The page `content/tools/<tool>.md` in the site, `evidence: true` and `wide: true`, placing
   `ev.agreement`, `ev.example`, `ev.band`/`ev.speed`, `ev.cases`; remove the tool's part-1
   figures from `static/figures/<tool>/`. Export, build (0 problems), run the number check, and
   look at every component: desktop and phone, light and dark, several slices and compare modes.

## The part-1 figures (no page uses them since part 2b)

`python -m larmorx_validation figures <tool>|all --out <site>/static/figures` draws a tool's
static figures from its records, benchmark reports and cache entries
(`larmorx_validation.figures`): SVG in both themes and two widths, text as outlines, brain
images as lossless WebP, each with a provenance JSON; a page places one with
`{{ figure('<tool>/<name>', 'caption') }}`. Their style (`figures/style.py`): the site's
fonts, programs in ink (the original), apricot (larmorx) and blue (a replica), signed
differences blue–paper–apricot on a stated symmetric scale. They are replaced tool by tool.

## Data

Only shareable data: OpenNeuro datasets whose `dataset_description.json` gives a public-domain
dedication (CC0-1.0, PDDL-1.0) or CC-BY (credited), and templates with their notices (the MNI
ICBM 152 notice). Brain slices only, masked, never a face. The cases used so far: ds000005 sub-01
(PDDL-1.0; T1w and BOLD, fMRIPrep 25.2.6's oracle run: denoising, N4, the brain extraction's
calls, the template registration, the T1w in MNI space, head-motion estimation; its `.h5`
composite; the NIfTI files of its run), ds005040 sub-002 (CC0-1.0; BOLD: slice timing, the
one-shot resampling with its field map), MNI152NLin2009cAsym (the MNI notice) and OASIS30ANTs
(CC-BY 4.0) from TemplateFlow.
