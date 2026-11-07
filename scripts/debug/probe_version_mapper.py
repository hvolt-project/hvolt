#!/usr/bin/env python3
"""探测 hvigor HosVersionMapper 的版本映射语义与数据源。"""
import re
import sys

path = sys.argv[1]
src = open(path).read()

print("== file size:", len(src))

for name in [
    "transferVersionIntoHosVersion",
    "getSupportVersions",
    "getAllVersion",
    "getHosVersion",
    "isSupportVersion",
]:
    i = src.find(name + "(")
    if i >= 0:
        print(f"== {name} ==")
        print(src[i : i + 520].replace("\\n", "\n"))
        print()

# 数据来源：内置映射表 or json 文件
for m in re.finditer(r'require\("([^"]*\.json[^"]*)"\)', src):
    print("JSON require:", m.group(1))
for m in re.finditer(r'(SUPPORT_VERSION[A-Z_]*|HOS_VERSION[A-Z_]*|SDK_VERSION[A-Z_]*)\s*=', src):
    print("const:", m.group(1))

# 打印任何含 "26.0" 的片段
for m in re.finditer(r'.{80}26\.0\.\d.{80}', src):
    print("CTX:", m.group(0)[:220])
