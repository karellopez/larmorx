#!/bin/bash
# SPDX-License-Identifier: Apache-2.0
# `python -m larmorx_validation <args...>` from a checkout, with its python/ and validation/src first.
# Usage: lxv.sh <checkout> <args...>   e.g. lxv.sh . parity ants-ai --tier standard --out /tmp/rec
source "$(dirname "$0")/env.sh"
W=$1; shift
export PYTHONPATH=$W/python:$W/validation/src
cd "$W" || exit 1
exec python -m larmorx_validation "$@"
