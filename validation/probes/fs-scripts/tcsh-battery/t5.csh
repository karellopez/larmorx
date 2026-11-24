#!/bin/tcsh -f
# SPDX-License-Identifier: Apache-2.0
echo "cwd=$cwd"
pushd $argv[1]
popd
