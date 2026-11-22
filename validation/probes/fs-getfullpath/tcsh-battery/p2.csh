#!/bin/tcsh -f
# SPDX-License-Identifier: Apache-2.0
set v = $argv[1];
echo "n=$#v"
foreach w ($v:q)
  echo "w=[$w]"
end
