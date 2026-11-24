#!/bin/tcsh -f
# SPDX-License-Identifier: Apache-2.0
mkdir -p sub/mri
pushd sub/mri
pushd $HOME
popd
popd
false | true
echo "st1 $status"
true | false
echo "st2 $status"
sh -c 'exit 3' | true
echo "st3 $status"
echo
echo -n
echo "x" >> /nonexistent/dir/f
echo "after"
