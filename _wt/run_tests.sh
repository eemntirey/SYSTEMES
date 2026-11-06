#!/usr/bin/env bash
set -euo pipefail
rm -rf "$HOME/kunit"
mkdir -p "$HOME/kunit"
cp -a /mnt/c/Users/eemntirey/Desktop/GlmOS/_wt/k27/kernel "$HOME/kunit/"
cd "$HOME/kunit/kernel"
echo "== host unit tests (all features) =="
"$HOME/.cargo/bin/cargo" test --all-features 2>&1 | tail -n 25
echo "== no_std target check =="
"$HOME/.cargo/bin/cargo" check --all-features --target x86_64-unknown-none 2>&1 | tail -n 4
