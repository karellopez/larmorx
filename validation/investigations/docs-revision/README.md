# The documentation revision: the old words

`old_words.py` counts, file by file, in this repository and in a checkout of the documentation
site (`larmorx_documentation`), the words that [docs/writing-guide.md](../../../docs/writing-guide.md)
replaces: *clean-room original*, *original* where it names a clean room, *exact replica*, *port*
as a kind of its own, the slogans, and the developers' jargon on the pages users read. Every
other *original* is counted as a word to read. It also lists the files whose name holds
*original* and how many files name each. The result is [inventory.md](inventory.md), from which
the revision's rollout is planned (docs/documentation-plan.md, step 4).

```
python -P validation/investigations/docs-revision/old_words.py --repo . \
    --site <checkout of larmorx_documentation> \
    --out validation/investigations/docs-revision/inventory.md
```

It reads tracked files only and takes about 15 seconds. Run again after each step of the rollout:
the columns of words to change should fall to zero in the areas the writer changes, and stay in
the areas kept as written (history, identifiers). Pages made by a measurement are told apart by
what wrote them (`MADE_PAGES`): a page of `docs/validation/` whose "Generated" line names a
`python -m larmorx_validation` command is a record (the clean-room audit's page has its own
area), and every benchmark report but the README is a report; these keep their words until each
is made again (the writing guide, section 1, "Where the old words stay"). A file named with
*original* that one of these pages links to is marked *renamed later*, with the pages that link
to it: it keeps its name until they are made again (section 1, "Renamed files").

`evidence_roles.py` counts what the site's evidence compares: every comparison label
(`role_label`) with its count and two examples, the panels' roles and the chart series' marks per
page. The writing guide's section 4 maps each label to its new form, and section 5 gives each
role and mark its colour.

```
python -P validation/investigations/docs-revision/evidence_roles.py <checkout of larmorx_documentation>
```

`pastels.py` checks the software colours of the site's Programs page (the writing guide, §5):
the pastels of tedana, sdcflows and fMRIPrep added in the Programs pilot against their neighbours
in the page's order (OKLab ΔE under normal vision, protanopia and deuteranopia), against every
other pastel (under normal vision and, at least 4, under protanopia and deuteranopia), the chip's
ink on each (WCAG 2's ratio), the new deep variants against the light paper, and their hue's
distance from the ports' blue and apricot. Every pair is printed under the four visions; today's
pairs and tritanopia are information only. It prints one line per check and PASS or FAIL.

```
python -P validation/investigations/docs-revision/pastels.py <checkout of larmorx_documentation>
```

## Moving an evidence page into the guide's words

Three helpers for the rollout's evidence pages (the part's notes say how a page moves). Each
reads a page's evidence JSON (`static/evidence/<page>.json` of a site checkout) and changes
nothing in it.

- `present_dry_run.py <module> <published json> <out json>`: applies the evidence module's
  `present` (its words, ports and order) to the published JSON, prints every worked example's
  What this shows line, panels, metrics groups, comparisons, facts and reasons, the charts'
  titles and the tables', and writes the result: a page's words read before it is exported.
- `evidence_texts.py <json>`: every text of the JSON with its pointer, for a search of the old
  words.
- `export_diff.py <before json> <after json>`: what an export changed: texts (counted), fields
  added, and every other difference (a number, a list's length, a key removed) listed. After an
  export of the revision it lists only texts and the fields `port`, `order`, `shows` and the
  presentation's, so no number moved.

- `summary_check.py <site>`: the Validation page's summary (`static/evidence/summary.json`)
  against every evidence JSON (cases and verdicts, each implementation by its record); run it
  after every export, and make the summary again (`python -m larmorx_validation evidence
  summary --site <site> --no-compute`) when it differs.
- `page_old_words.py <site>/_site/evidence/<page>/index.html`: the old words left in a built
  page's visible text, tooltips included, each distinct text once with its count, for a reader
  to judge (a message's `larmorx:` prefix, the provenance's commits and the random-testing
  panel's texts stay until their chunks). Inside code it lists the placeholders and paths that
  name the oracle, the workspace or the figures cache (`code` lines), which a command should
  not show, and every paragraph, list item or cell that offers `--upstream-defects`,
  `--n-threads` or `--implementation` without "next release" (`option` lines: 0.1.0rc1 has
  only the environment variables).
- `panel_runs_check.py <site>`: every fMRIPrep run an evidence page names in its data panel's
  cases and worked examples is one of the panel's fourteen (`fmriprep_nodes.PANEL_RUNS`); run it
  after every export, beside `summary_check.py`. Since relay 15 `fmriprep_nodes` reads only
  those fourteen runs, so an export no longer needs `LARMORX_FMRIPREP_RUNS` to leave out the
  runs of 2026-11-08 (an export of relay 12 had named three of their nodes on DenoiseImage's
  page).

```
PYTHONPATH=python:validation/src python -P validation/investigations/docs-revision/present_dry_run.py \
    ants_n4 <site>/static/evidence/ants-n4.json /tmp/ants-n4.json
python -P validation/investigations/docs-revision/export_diff.py old.json new.json
```
