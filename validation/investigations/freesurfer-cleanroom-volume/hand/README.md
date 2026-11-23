# Hand cases of the fs.volume clean room (black box)

Small cases the relays wrote to settle a reading of `specs/freesurfer-volume.md` that the probe's
groups do not show, run on the prebuilt probe and on the candidate alike (never the probe's
source). Each script says what it shows and how to run it; run it inside a memory box
(`scripts/orchestration/boxed.sh`).

| Script | Shows | Result (relay) |
|---|---|---|
| `conform_hash.py` | the probe's voxel hash of `conform` (README: "FNV-1a 64") on inputs of known bytes | the usual prime from the start state 1469598103934665603 (the usual offset basis without its last digit) fits all five; probe and candidate the same (relay 20) |
| `dilate_empty_box.py` | §48: the dilations' box when the search finds nothing (an all-zero source into a destination of nonzero values) | an extent of 1 in all three axes visits its voxel, larger extents none: the box is not grown when empty; probe and candidate the same on all 13 cases (relay 20) |
