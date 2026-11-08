#!/bin/bash
# SPDX-License-Identifier: Apache-2.0
# Copy pybids 0.20.0's sources (the fMRIPrep image's) into the scratch folder, and diff the parts
# sdcflows' wrangler reaches against pybids 0.22.0 (newest).
S=${OUT:?set OUT to an output folder}
IMG=${LARMORX_WORKSPACE:?}/oracles/images/fmriprep-25.2.6.sif
mkdir -p "$S/image-src/pybids-0.20.0"
timeout 600 apptainer exec --cleanenv -B "$S/image-src/pybids-0.20.0":/out "$IMG" bash -c '
SP=/app/.pixi/envs/fmriprep/lib/python3.12/site-packages
cp -r $SP/bids /out/
'
find "$S/image-src/pybids-0.20.0" -name '*.pyc' -delete
N=${LARMORX_WORKSPACE:?}/tags/pybids-0.22.0/src/bids
O="$S/image-src/pybids-0.20.0/bids"
diff -rq "$O" "$N" | grep -v tests | grep -v __pycache__
for f in layout/config/bids.json layout/index.py layout/utils.py layout/models.py; do
  echo "== $f: $(diff "$O/$f" "$N/$f" | grep -c '^[<>]') changed lines"
done
