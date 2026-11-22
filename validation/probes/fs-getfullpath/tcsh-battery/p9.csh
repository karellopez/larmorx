#!/bin/tcsh -f
# SPDX-License-Identifier: Apache-2.0
set b = `/bin/echo $argv[1]:q`
echo "n=$#b"
foreach w ($b:q)
  echo "w=[$w]"
end
