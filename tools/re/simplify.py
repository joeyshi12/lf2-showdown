#!/usr/bin/env python3
"""Rewrite inlined MSVC string idioms in a Ghidra decompile so it is shorter to read.
usage: simplify.py FILE.c  (prints to stdout)"""
import re, sys
src = open(sys.argv[1]).read()
# inlined strcmp of a fixed literal (repe cmpsb)
pat = re.compile(
    r'(?P<i>\w+) = (?P<n>\w+);\n\s*(?P<b>\w+) = true;\n\s*(?P<pa>\w+) = (?P<a>[^;]+);\n'
    r'\s*(?P<pb>\w+) = (?P<lit>"(?:[^"\\]|\\.)*"|[^;]+);\n\s*do \{\n\s*if \((?P=i) == 0\) break;\n'
    r'\s*(?P=i) = (?P=i) \+ -1;\n\s*(?P=b) = \*(?P=pa) == \*(?P=pb);\n'
    r'\s*(?P=pa) = (?P=pa) \+ 1;\n\s*(?P=pb) = (?P=pb) \+ 1;\n\s*\} while \((?P=b)\);')
src = pat.sub(lambda m: f'{m["b"]} = streq({m["a"]}, {m["lit"]});', src)
# inlined strlen walk
pat2 = re.compile(r'do \{\n\s*(\w+) = \*(\w+);\n\s*\2 = \2 \+ 1;\n\s*\} while \(\1 != \'\\0\'\);')
src = pat2.sub(lambda m: f'/* {m[2]} = end of string + 1 */', src)
# abs() from the lf2_abs call-fixup: v = X, s = (int)v >> 0x1f, (int)((v ^ s) - s)
pat3 = re.compile(r'(\w+) = ([^,;]+?),\s*(\w+) = \(int\)\1 >> 0x1f,\s*((?:(?!\b\1 = |\b\3 = ).)*?)\(int\)\(\(\1 \^ \3\) - \3\)', re.S)
for _ in range(20):
    src2 = pat3.sub(lambda m: f'{m[4]}abs({m[2]})', src)
    if src2 == src: break
    src = src2
src = re.sub(r'\(int\)\(longlong\)', 'ftol ', src)
print(src)
