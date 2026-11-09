# `lx.bids`: a BIDS dataset index (pybids' `BIDSLayout`, without its database)

`larmorx.bids` indexes a BIDS dataset as pybids 0.22.0 does (`python/larmorx/bids/layout.py`;
pybids is MIT, Copyright (c) 2016-2026 The PyBIDS Developers): the same files, the same entities
of each file name (pybids' `bids.json` and `derivatives.json`, niworkflows' `nipreps.json`,
shipped unchanged in `python/larmorx/bids/config/`), the same metadata (each file's JSON
sidecars merged by pybids' inheritance) and the same query results, in memory: no SQLite
database, no SQLAlchemy, no `bids-validator`, `formulaic` or `pandas` (decision D3 of the field
maps' part B). sdcflows' estimator search (`lx.sdcflows.find_estimators`) and fMRIPrep's
field-map selection (`larmorx.pipelines.larmorprepx.estimators`) query it.

```python
import larmorx as lx

layout = lx.bids.BIDSLayout("/data/ds005040", validate=False)
layout.get_subjects()                                  # ['002']
bold = layout.get(subject="002", suffix="bold", extension=".nii.gz")
bold[0].get_metadata()["PhaseEncodingDirection"]       # its sidecars, inherited
layout.get(suffix="epi", IntendedFor=lx.bids.Query.REQUIRED)
```

## Options and the original

| larmorx | pybids 0.22.0 | Notes |
|---|---|---|
| `BIDSLayout(root, validate=True, *, derivatives=False, config=None, sources=None, regex_search=False, indexer=None, is_derivative=False, pybids_version="0.22.0")` | `BIDSLayout(root, validate, derivatives, config, sources, regex_search, database_path, reset_database, indexer, is_derivative)` | `validate`: the root's `dataset_description.json` checks (pybids' messages); `config`: `"bids"`, `"derivatives"`, a JSON file (`lx.bids.NIPREPS_CONFIG`) or a dictionary (`"bids-schema"`: `NotImplementedError`); `pybids_version="0.20.0"` (fMRIPrep 25.2.6's image): `get_entities`' order and no named-dataset warning. No `database_path`/`reset_database` (no database); after `validate`, keywords only |
| `BIDSLayoutIndexer(validate=False, ignore=None, force_index=None, index_metadata=True, config_filename="layout_config.json", **filters)` | the same | `ignore`/`force_index`: paths relative to the root (as `^/<path>.*`) or compiled expressions searched in `/`-rooted relative paths; default `^/(code|models|sourcedata|stimuli)`; dot files always ignored; `derivatives/` never walked; per-folder `layout_config.json`; `validate=True` (the BIDS validator's file rules): `NotImplementedError` |
| `layout.get(return_type="object", target=None, scope="all", regex_search=False, invalid_filters="error", **filters)` | the same | `"object"`, `"file"`/`"filename"`, `"id"` (with `target`), `"dir"`; filter values: a value, a list (any), `None`/`Query.NONE` (absent), `Query.REQUIRED`/`ANY` (present), `Query.OPTIONAL` (no filter), `[]` (no filter); `regex_search`: case-insensitive `re.search` of every filter (SQLite's `REGEXP` as pybids registers it); an `int` filter compares the stored text as SQLite's `CAST(... AS INTEGER)` (`run=1` matches `run-01`), a string as text (`run="1"` does not); `extension` gets its leading dot (with the `bids` configuration); results in pybids' natural order |
| `layout.get_<entity>s(**filters)` (`get_subjects`, `get_sessions`, `get_B0FieldIdentifiers`, ...) | the same | `get(return_type="id", target=<entity>)`; the plural's singular by English's regular rules (pybids asks `inflect`); an unknown entity: `BIDSEntityError` |
| `layout.get_file(filename, scope="all")`, `layout.get_metadata(path, include_entities=False, scope="all")`, `layout.get_entities(scope="all", metadata=None)`, `layout.parse_file_entities(...)`, `layout.add_derivatives(path, **kwargs)`, `layout.get_files()`, `layout.root`, `layout.entities`, `layout.files`, `layout.derivatives`, `layout.description` | the same | `get_metadata`'s keys ordered by name, as pybids' database returns them; a path the layout does not hold gives `{}` |
| `BIDSFile` (`BIDSImageFile`, `BIDSDataFile`, `BIDSJSONFile`): `path`, `filename`, `dirname`, `relpath`, `entities`, `get_entities(metadata=False, values="tags")`, `get_metadata()`, `get_dict()`/`get_json()` (JSON) | the same | one object per file (identity as pybids' session gives); `entities` (file name and metadata) ordered by name; `get_entities` in BIDS schema order (bidsschematools 2.0.0's, pybids 0.22.0) or by name (0.20.0) |
| `parse_file_entities(filename, entities=None, config=None, include_unmatched=False)`, `Query`, `Config`, `Entity`, `PaddedInt`, `BIDSMetadata`, `listify`, `natural_sort` | the same | integer entities (`run`) and integer metadata come back as `PaddedInt`, as pybids reads them from its database |
| errors: `PyBIDSError`, `BIDSEntityError` (an `AttributeError`), `TargetError`, `BIDSValidationError`, `BIDSDerivativesValidationError`, `BIDSConflictingValuesError` (`ValueError`s) | the same | the same messages; a sidecar that is not JSON: `OSError("Error occurred while trying to decode JSON from file <BIDSJSONFile filename='...'>")` |

## The details pybids' database decides, kept

- Entities are searched in the **whole** path (pybids' patterns on the absolute path): a folder
  above the dataset named `sub-x` or `ses-y` gives its value to files without one of their own.
- Metadata inheritance: every sidecar of the file's suffix in its folder and each folder above
  whose entities are a subset of the file's, merged from the farthest; within one folder the
  sidecars are applied in reverse natural order, so the *less* specific sidecar
  (`sub-01_task-rest_bold.json`) wins over `sub-01_task-rest_run-01_bold.json`; `null` values
  are dropped; a metadata key that is also a file-name entity with another value is a
  `BIDSConflictingValuesError`, with the same value it is not metadata.
- The `IntendedFor` checks of the file associations run (a file without a subject entity whose
  sidecar has `IntendedFor` is pybids' `KeyError`; pybids 0.22.0 warns "Skipping association for
  None" for each `bids:<name>:` URI); the associations themselves are not kept (nothing on the
  field-map paths reads them).

## Differences

- pybids matches its ignore patterns against relative paths with the platform's separator, so
  on Windows `^/code` and fMRIPrep's patterns never match; larmorx always writes `/` (pybids'
  result on Linux and macOS, on every platform).
- Ties of pybids' natural sort of unique values (`'01'` and `'1'`) and of `invalid_filters`'
  first bad name follow a Python `set`'s order in pybids (it changes between processes); larmorx
  breaks them by plain string order. Not reachable on a valid dataset (BIDS labels are unique).
- Not ported (nothing on fMRIPrep's or sdcflows' field-map paths calls them): file associations
  (`get_associations`, `get_fieldmap`, `get_nearest`, `get_bval`, `get_bvec`, `get_tr`),
  `build_path`, `write_to_file`, `copy_files`, `get_collections`, `to_df`, `clone`, `save`/`load`
  and database files, the `bids-schema` configuration, `BIDSImageFile.get_image`,
  `BIDSDataFile.get_df`, `get_dataset_description`.

## Validation

`validation/investigations/sdcflows-fieldmap-b/check_estimators.py` (`index`, `image`):
against pybids 0.22.0 (both kept environments) and 0.20.0 (fMRIPrep's image) on the BIDS
inputs of the fMRIPrep oracle runs, sdcflows' test datasets, sdcflows' generated skeletons and
edge-case trees (inheritance, `bids::` URIs, links, a broken link, a linked folder, hidden and
ignored folders, a derivatives dataset, conflicting and invalid sidecars), with fMRIPrep's,
sdcflows' app's, sdcflows' tests' and pybids' default indexers: every file's entities and
metadata (values, types, order), the layout's entities, the `get_<entity>s` values, 42 queries
per layout and every call sdcflows' search makes.
