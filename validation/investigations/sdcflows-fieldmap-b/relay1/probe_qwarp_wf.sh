#!/bin/bash
# SPDX-License-Identifier: Apache-2.0
# Build sdcflows' workflows in the newest environment (no execution), offline templates.
S=${OUT:?set OUT to an output folder}
W=${LARMORX_WORKSPACE:?}
mkdir -p "$S/probe-wf"
cd "$S/probe-wf" || exit 1
export TEMPLATEFLOW_HOME="$W/oracles/templateflow"
export TEMPLATEFLOW_AUTOUPDATE=0
export HOME="$S/probe-wf"
timeout 300 "$W/oracles/python-newest/bin/python" -I "$S/scripts/probe_qwarp_wf.py" 2>&1 | grep -v 'DeprecationWarning\|warnings.warn' | cut -c1-1500
