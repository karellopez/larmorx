#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
x=1
/usr/bin/python3 -c "import os; os.abort()" a $x
echo next $?
