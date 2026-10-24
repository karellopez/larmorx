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
| `<tool>`, `summary`, `panel` or `all` | a tool of `larmorx_validation.evidence.TOOLS`; `summary` writes the Validation page's matrix of every tool; `panel` the Data panel page's data (`static/evidence/panel.json`, from larmorx-testdata's `panel.toml` and catalog); `all` does every tool, then the summary and the panel |
| `sizes` | prints the published site's size, page by page, against the budgets (below), and fails when one is exceeded; writes nothing |
| `--site DIR` | the site's checkout: writes `static/evidence/<tool>.json`, `static/evidence/summary.json`, the images in `static/evidence/blobs/` and their manifest `static/evidence/blobs.sha256`, and removes the blobs no JSON refers to any more |
| `--cache DIR` | the figures cache (default: `LARMORX_FIGURES_CACHE`, else the workspace's `oracles/figures-cache`) |
| `--no-compute` | never compute a missing cache entry: skip the tools that need one |

Every export ends with the **size guard**: it prints the table of `evidence sizes` and fails
(exit status 1, naming the largest pages and examples) when the published site would exceed
**500 MB** (GitHub Pages publishes at most 1 GB) or one page **40 MB** (its JSON and the images
it names; about 25 tool pages fit the site at 20 MB on average: two or three key examples as
whole volumes, the rest as slices). Every export also attaches the **credits** (below) and
refuses an example whose *Data* fact states another licence than the catalog's.

**The images are not in the site's `main` branch.** `static/evidence/blobs/` is git-ignored
there; the manifest `blobs.sha256` (every blob a JSON names, with its full SHA-256) is committed
with the JSON, and the blobs themselves are published from the orphan branch `evidence-blobs`:
one parentless commit holding exactly the manifest's blobs, replaced at every export
(`python tools/blobs.py commit` in the site, then `git push --force origin evidence-blobs`
before `main`). The Pages workflow checks that branch out, verifies every blob against the
manifest (a missing, extra or altered blob stops the deployment) and publishes them with the
site; a local build fetches them first (`python tools/blobs.py fetch`). So the site's history
grows by the JSON only, and a superseded export's images leave the repository when the branch is
replaced. The site's README, "The validation evidence", has the commands.

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
  shown brain differs most. A mask is drawn over the image it was made from (a panel's
  `overlay`, `Panel(..., overlay={output: {"on": panel, "window": [lo, hi]}})`; the still
  poster the same, `example.poster_overlay`; 3dAutomask's page). Each comparison's verdict is decided by code from these metrics
  (e.g. *identical with correctly rounded maths* only when larmorx − original-with-CORE-MATH is
  bit-identical), never typed; the chain of causes asserts every identity it shows.
- **The display data**: the file's voxels reoriented to RAS (flips and transposes only) and
  cropped to the box around the shown region plus a margin of 2 voxels (the display grid; its
  `to_file` matrix maps every display voxel back to the file's, so coordinates, crosshairs,
  overlays and hover values stay exact), zero outside the region; stored as raw little-endian
  values in byte planes, zlib-compressed, or as the exclusive or of their bits with a reference
  (another panel, or a float32 quotient such as N4's input ÷ bias field), or as `same`/`neg` of
  another panel, whichever is smallest: lossless, never JPEG or PNG, so the viewer reads exact
  values. **Integer data stays integer:** a panel ships its file's own type (int16, uint8, …;
  half the size of float32 for int16), and an integer file with a scale factor ships its stored
  integers with the scale and intercept (`common.stored(path)` returns them for
  `Panel(..., scale=, inter=)`; the viewer shows stored × scale + intercept, as nibabel does).
  Blobs are named by their SHA-256 (byte-deterministic with this machine's zlib), so an unchanged
  result is the same file. Still posters (lossless WebP) of the default view serve readers
  without JavaScript.
- **The shown region**: the brain (fMRIPrep's mask, dilated) by default. The whole head
  (`common.head_mask(data)`: the head above a low threshold, holes filled, dilated; the box is
  then the head's) only for images whose defacing was checked: larmorx-testdata's `panel.toml`
  gives each anatomical image of the data panel `show = "head"` or `"brain"`; an image whose face
  could be recognised is shown brain-masked only, and so is everything computed from it.
- **Views** (`Example(ctx, grid, show, view=...)`): `"volume"` ships the whole display grid
  (every slice, for the two or three key datasets of a tool); `"slices"` ships three
  full-resolution orthogonal slices, one per plane, through one voxel, each cropped to the shown
  region in that plane plus 2 voxels, the arrays stored as the slices one after the other
  (axial, coronal, sagittal; for a series, volume after volume) with each plane's box and offset
  in the example's `view`, so the viewer maps a voxel to its value exactly. The voxel is the
  default view's (`common.finish` cuts there): `common.slice_voxel(ex, a, b, output)` gives the
  voxel where the main comparison differs most over the whole shown region, or the region's
  centre where nothing differs. Every metric is still computed offline over every voxel of the
  files; only *where the shown data differ most* (the "go to the largest difference" button and
  the viewers' check) is sought in the three slices. The model: N4's ANTS-40 example
  (`corrections.Spec(..., view="slices")`), a 0.8 mm 7T image that took 40 MB as a volume and
  takes 0.7 MB as slices.
- **Credits** (`evidence/credits.py`): every dataset an example names (an OpenNeuro accession, a
  TemplateFlow template) gets its credit line from larmorx-testdata's catalog (title, version,
  DOI, licence, citation): the example's `credits` and the page's `credits`, `cases_credits` for
  the cases and tables. The site prints them under each example and at the page's end: for
  CC BY data a proper attribution with the changes made, for CC BY-SA data (ds001600) also the
  share-alike notice for the images adapted from it.
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
   or `itk_hdf5.py` (no images: tables), `afni_automask.py` (a replica only, masks drawn over
   their input; its helpers run AFNI, fMRIPrep's AFNI and the replica) as the model, and
   `common.py`'s pieces. Give each example its own copy of a module-level outputs list
   (`copy.deepcopy`): `common.finish` writes the windows into it.
2. The cache entry: the pipeline's real call on real data where there is one (an fMRIPrep
   oracle run's node), the original as built, with CORE-MATH, at the thread counts its record's
   band uses, fMRIPrep's own build if it differs; larmorx in both modes and at 1 and 12 threads.
   Keep it to minutes.
3. The view: `view="volume"` for the tool's two or three key panel datasets, `view="slices"`
   (cut at `common.slice_voxel`) for the other panel cases; panels in their files' own types
   (`common.stored`); the head only where `panel.toml` allows it. The page must stay within the
   40 MB budget (`evidence sizes`).
4. The example: panels (`Example.add`; order them so the closest references come first), the
   comparisons (`_diffs`), their verdicts from the metrics, the chain of causes where larmorx
   and fMRIPrep differ, a default view worth seeing (a slice through tissue where the tool
   acts, the crosshair on a difference), the facts (data, node, command, programs, grid) and
   the provenance. A D13 correction gets an example of its own where a real input can carry the
   defect (as AFNI-01's re-stored run).
5. The cases: each case's verdict from the record (re-generate the record from a clean commit if
   it predates the *reported* values), its description and command line from the suite.
6. The summary: move the tool in `evidence/summary.py` from its part-1 figure to its evidence.
7. The page `content/tools/<tool>.md` in the site, `evidence: true` and `wide: true`, placing
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
dedication (CC0-1.0, PDDL-1.0) or CC-BY or CC-BY-SA (credited, with the share-alike notice), and
templates with their notices (the MNI ICBM 152 notice); every credit line comes from
larmorx-testdata's catalog. Never a recognisable face: brain-masked views, or the head of an
image whose defacing was checked (`panel.toml`, `show = "head"`).

**The data panel** (larmorx-testdata `docs/panel.md`, approved 2026-10-24): fourteen subjects,
the fixed set every tool is to be validated and shown on (`python -m larmorx_testdata fetch
--tier full --tag panel`; `larmorx_testdata.panel()` lists them with their roles, criteria and
fMRIPrep oracle runs). A tool's page shows two or three of them as volumes and the others as
slices. The site's Data panel page (`content/data-panel.md`, from `evidence panel`) describes
them. The cases used so far: ds000005 sub-01
(PDDL-1.0; T1w and BOLD, fMRIPrep 25.2.6's oracle run: denoising, N4, the brain extraction's
calls, the template registration, the T1w in MNI space, head-motion estimation; its `.h5`
composite; the NIfTI files of its run; its raw T1w: 3dUnifize), ds005040 sub-002 (CC0-1.0;
BOLD: slice timing, the one-shot resampling with its field map; its BOLD reference: 3dUnifize
and 3dAutomask as fMRIPrep calls them; the raw series: 3dAutomask), MNI152NLin2009cAsym (the MNI notice) and OASIS30ANTs
(CC-BY 4.0) from TemplateFlow.
