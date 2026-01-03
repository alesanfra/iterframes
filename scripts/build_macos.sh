#!/usr/bin/env bash
set -x
set -e

echo "Build wheel"
maturin build --strip --release

echo "Fix shared libs"
delocate-wheel -v target/wheels/iterframes-*-abi3-macosx_*.whl

echo "Check wheel"
delocate-listdeps target/wheels/iterframes-*-abi3-macosx_*.whl
