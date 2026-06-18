#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
# Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
#
# Build every per-control example project with rcrun and report pass/fail.
# Usage: examples/build-all.sh        (run from the repo root)
set -u
cd "$(dirname "$0")/.." || exit 1

pass=0; fail=0; fails=""
for d in examples/*/; do
  [ -f "${d}cobolt.toml" ] || continue
  name=$(basename "$d")
  if cargo run -q -p cobolt-cli -- build "${d}cobolt.toml" >/dev/null 2>&1; then
    pass=$((pass + 1))
    printf '  ok   %s\n' "$name"
  else
    fail=$((fail + 1)); fails="$fails $name"
    printf '  FAIL %s\n' "$name"
  fi
done

echo
echo "RESULT: $pass passed, $fail failed"
[ -n "$fails" ] && echo "FAILED:$fails"
[ "$fail" -eq 0 ]
