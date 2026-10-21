# Figures: the visual evidence of every tool

Every tool's contract includes figures (CLAUDE.md rule 7): how the port, the clean-room
original and the replica compare with the original program, drawn from the committed records
and published on the documentation site (`larmorx_documentation`, one page per tool under
*Tools*). The code that draws them lives here, in `larmorx_validation.figures`, so every figure
can be drawn again from a commit of larmorx; the rendered files live in the site's repository.
Figures never run in CI (rule 10).

## Run

```bash
pip install -e "validation[figures]"   # matplotlib, fonttools, nibabel, scipy
# larmorx itself: a release build of this checkout (maturin build, or maturin develop in the
# main checkout); the oracles named in CLAUDE.md for the image figures
python -m larmorx_validation figures ants-n4 --out ../larmorx_documentation/static/figures
python -m larmorx_validation figures all --out ../larmorx_documentation/static/figures
```

| Option | Effect |
|---|---|
| `<tool>` or `all` | a page's figures (`larmorx_validation.figures.TOOLS`), or every page's |
| `--out DIR` | writes `DIR/<tool>/<figure>-{light,dark}[-narrow].svg` and `DIR/<tool>/<figure>.json` |
| `--only NAME ...` | only these figures of the tool |
| `--cache DIR` | the figures cache (default: `LARMORX_FIGURES_CACHE`, else the workspace's `oracles/figures-cache`, else `~/.cache/larmorx-figures`) |
| `--no-compute` | never compute a missing cache entry; the figures that need one are skipped (exit status 1) |

A tool's figures take seconds to draw from the records. An image figure needs real outputs:
the first run computes them into the cache (minutes: the original program, at one thread and
at others where its thread band is the story, with CORE-MATH preloaded, and larmorx), and later
runs reuse them.

## What a figure is made of

- **Records.** The agreement overviews, the differences against the original's thread band and
  the speed charts read `docs/validation/<suite>.json` and `.md` and `docs/benchmarks/<suite>.json`
  (`figures/records.py`). The Markdown records' tables are read by their column names; a figure
  fails loudly when the table it needs is missing. Numbers are never typed into figure code.
- **The cache** (`figures/cache.py`): `<cache>/<tool>/<case>/` holds the outputs of the original
  program and of larmorx on one or two real cases per tool, and `RUN_INFO.json`, written last:
  every command with its environment, threads, seed, exit status and time, the program versions,
  the larmorx commit and the inputs' SHA-256. An entry without `RUN_INFO.json` is computed again.
  Never commit cache contents (image data).
- **Provenance** (`<figure>.json` beside the files): the records (path, SHA-256, the commit
  they were generated from), the cache entries (commands, versions), the data (dataset, subject,
  licence, files; the MNI notice where the template is shown), the larmorx commit the figure was
  drawn from, the command and the package versions. The site links it under each figure.

## The style

One style for every figure (`figures/style.py`), the site's design language:

- **Two themes, two widths.** Each figure is drawn for the light and the dark theme, on the
  colour of the site's figure card (`--bg-elev`), at the text column's width (720 px) and a
  phone's (360 px). The site shows the version of its current theme (the switch and the system
  setting) and the narrow one below 560 px.
- **Type.** Instrument Serif for numerals that matter, Inter for text, JetBrains Mono for labels
  and ticks, fetched once from Google Fonts' repository at a pinned commit (`figures/fonts.py`,
  SIL Open Font License) and drawn as outlines, so the SVG shows them anywhere.
- **Colour has one job per chart.** Programs: the original in ink, larmorx in apricot, a replica
  in blue, a second reference (e.g. SimpleITK) in grey. Agreement states: identical to the
  unmodified original in apricot, identical with correctly rounded maths in apricot with a hatch,
  within the record's thresholds in blue, an expected divergence in ochre, a failure in magenta,
  rejected by both in a hatched grey. Signed differences: blue, the paper at zero, apricot, on a
  symmetric scale whose range the figure states. Magnitudes: from the paper to apricot. The
  palette passes a colour-vision-deficiency check in both themes (neighbours ΔE ≥ 13 under
  protanopia and deuteranopia, normal vision ΔE ≥ 15, 3:1 against the card); identity never
  rests on colour alone (legends, hatches, labels).
- **Brain images.** Slices only, masked to a slightly dilated brain mask on a transparent
  background, so no slice can show a face; neurological convention, true voxel sizes; a
  hairline outline of the brain on difference maps; a difference map that is zero says
  "bit-identical". Rasters are embedded at their own resolution as lossless WebP.
- **Honesty.** `experimental` tools are shown as experimental; expected divergences and D13
  corrections are drawn as what they are; a tool without a benchmark report gets no speed chart
  (its page says the benchmark is pending): no charts from informal timings.

## Data

Only shareable data: OpenNeuro datasets whose `dataset_description.json` gives a public-domain
dedication (CC0-1.0, PDDL-1.0) or CC-BY (credited), and templates with their notices (the MNI
ICBM 152 notice). The provenance names the dataset, subject and licence of every image shown.
The cases used so far: ds000005 sub-01 (PDDL-1.0; T1w, BOLD run 1, fMRIPrep 25.2.6's oracle
run), ds005040 sub-002 (CC0-1.0; BOLD with slice timing, PEPOLAR field map), MNI152NLin2009cAsym.

## Add a tool

1. Create `validation/src/larmorx_validation/figures/<tool>.py` with
   `figures(ctx) -> list[Figure]`, and add it to `TOOLS` in `figures/__init__.py`.
2. Start from what the record gives, with the shared figures in `figures/common.py`:
   - `agreement_figure(rec, split=...)`: one bar per category; `split` says how a category's
     passing cases divide among identical to the unmodified original, identical with
     correctly rounded maths, and within the thresholds (`split_by_cases`,
     `split_by_category_table`, or the record's own sentences);
   - `band_figure(rec, ...)`: an ANTs record's differences divided by the original's thread
     band;
   - `speed_figure(bench, series=..., labels=...)`: one row per input, short labels.
3. Add what is particular to the program: an image comparison on a real case (a cache entry
   computed by `cache.entry(ctx, tool, case, description, compute)`, drawn with
   `brain.Grid`), time series, a variability band. Compute what the record compared (the same
   command lines, `upstream_defects` in `reproduce` mode, the original at one thread), plus
   the original's own variability, and keep each case to minutes.
4. Draw it, then look at it: both themes and both widths (`rsvg-convert -b '#f0ebe0' x.svg -o
   x.png`, or the site's review screenshots). Fix overlaps and clipped labels before
   publishing; keep each file small (the text outlines and the images are most of it).
5. Publish on the site (`larmorx_documentation`): the files in `static/figures/<tool>/`, a page
   `content/tools/<tool>.md` that places them with `{{ figure('<tool>/<name>', 'caption') }}`
   (what is shown, on which data), its entry in `site.yml` (`tools`) and the status table's link
   (`data/status.yml`, `page:`); `tools/build.py` must report 0 problems. A record that changes
   means its figures are drawn again from the commit that has it.
