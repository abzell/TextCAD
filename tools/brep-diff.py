#!/usr/bin/env python3
"""Compares two directories of canonical B-reps (BREP_T1_DUMP) from different platforms.

    python3 tools/brep-diff.py DIR_A DIR_B

For each part that differs: how many lines differ, the first few differing lines side by side, and
the largest relative difference between numbers on those lines (a last-bit difference in a
coordinate shows up as ~1e-16).
"""
import os
import re
import sys

NUM = re.compile(r"[-+]?\d+\.?\d*(?:[eE][-+]?\d+)?")


def lines(path):
    with open(path, "rb") as f:
        return f.read().decode("latin-1").replace("\r\n", "\n").split("\n")


def main(a_dir, b_dir):
    for d in (a_dir, b_dir):
        if not os.path.isdir(d):
            print(f"{d}: no B-reps (that build did not get that far)")
            return
    names = sorted(set(os.listdir(a_dir)) & set(os.listdir(b_dir)))
    differ = 0
    for name in names:
        a, b = lines(os.path.join(a_dir, name)), lines(os.path.join(b_dir, name))
        if a == b:
            continue
        differ += 1
        pairs = [(i, x, y) for i, (x, y) in enumerate(zip(a, b)) if x != y]
        worst = 0.0
        same_shape = True
        for _, x, y in pairs:
            nx, ny = NUM.findall(x), NUM.findall(y)
            if len(nx) != len(ny):
                same_shape = False
                continue
            for u, v in zip(nx, ny):
                u, v = float(u), float(v)
                if u != v:
                    worst = max(worst, abs(u - v) / max(abs(u), abs(v), 1e-300))
        print(f"== {name}: {len(pairs)} of {len(a)}/{len(b)} lines differ, "
              f"largest relative difference {worst:.3g}"
              + ("" if same_shape and len(a) == len(b) else ", structure differs"))
        for i, x, y in pairs[:4]:
            print(f"  line {i + 1}\n    A {x[:160]}\n    B {y[:160]}")
    print(f"{differ} of {len(names)} parts differ")


if __name__ == "__main__":
    main(*sys.argv[1:3])
