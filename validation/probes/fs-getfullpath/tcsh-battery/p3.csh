#!/bin/tcsh -f
# SPDX-License-Identifier: Apache-2.0
set b = `echo $argv[1]`
echo "n=$#b"
foreach w ($b:q)
  echo "w=[$w]"
end
