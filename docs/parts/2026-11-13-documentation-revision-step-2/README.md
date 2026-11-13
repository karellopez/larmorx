# The documentation revision, step 2 (2026-11-13)

Step 2, reduced by the user's decision of 2026-11-13 to what the site's readers see and run without a separate
reviewer (decision log, "2026-11-13 (evening)"). Published in two merges:

- **Step 2a** (published early on the user's request): FreeSurfer's detailed roadmap page (`/roadmap/freesurfer/`,
  its statuses in the site's `data/freesurfer-roadmap.yml`, updated at each FreeSurfer merge), the confounds' evidence
  page with its rows on the Programs and Validation pages, and the first reference pages (Q1 to Q4 so far). larmorx
  `4fc362e4` (merge of `75722946`) and `3b532e71` (audit); site `3c9ffd2`, evidence-blobs `fbd1c67` (1,501 blobs).
- **Step 2:** a reference page for each of the 46 programs with a command, the wrappers' `n_threads` everywhere (rule
  5), their printed messages in the new words, the porting guide's §11 and `docs/figures.md` naming the writing guide
  and the shared presenter. larmorx `8b440d34` (merge of `518913e7`), `a388353b` (handoff), `052713f4` (audit); site
  `1d7db52`.

Moved to the final validation phase: the repository's other documents, the findings', the code's and the tests'
words, `programs.py`'s rule for `validated`, the clean rooms' printed lines in the old words, the site's small layout
items, `n_threads` in the function libraries.

- `brief.md`: the writer's brief; `notes.md`: the relay notes (relays 1 to 34; paths as placeholders).
- `handoff.md`, `handoff.toml`: step 2's handoff (2 ops, applied at a388353b).
- `report.md`: the final reports of relays 29 to 34. Step 1's record: `../2026-11-13-documentation-revision-step-1/`.
