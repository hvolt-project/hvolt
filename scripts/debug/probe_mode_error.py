#!/usr/bin/env python3
"""在 ohos/hos SDK handler 里定位 00308018 mode-changed 的触发条件。"""
import re
import sys

path = sys.argv[1]
src = open(path).read()

anchor = src.find("mode has changed")
if anchor < 0:
    for m in re.finditer(r"00308018|[Mm]ode.{0,30}changed", src):
        anchor = m.start()
        break
print("== file:", path, len(src))
print("== context around anchor ==")
print(src[max(0, anchor - 2200): anchor + 400])
