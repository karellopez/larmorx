#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
# The FSL pages' evidence (BET and head-motion correction) and the summary exported into a site
# checkout, each export in a memory box; its new figures-cache entries (FSL 6.0.7.23's runs:
# evidence/fsl_step.py) are computed on the way. Then the site's status table (`tables --site`).
# Usage: export_site.sh <site checkout> <log dir>
set -u
here=$(cd "$(dirname "$0")" && pwd)
site=$1
logs=$2
mkdir -p "$logs"
for t in fsl-bet mri-hmc summary; do
  "$here/boxed_lxv.sh" 24 "$logs/export-$t.log" evidence "$t" --site "$site"
  echo "== $t: $(tail -1 "$logs/export-$t.log")"
  grep -E "Traceback|Error|exceeds|over budget" "$logs/export-$t.log" | tail -5
done
"$here/boxed_lxv.sh" 4 "$logs/tables-site.log" tables --site "$site"
echo "== tables: $(tail -1 "$logs/tables-site.log")"
echo "EXPORT-DONE"
