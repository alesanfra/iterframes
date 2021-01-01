#!/usr/bin/env bash
set -x

echo "Build wheel"
maturin build --strip --release

echo "Fix shared libs"
delocate-wheel -v target/wheels/iterframes-*-abi3-macosx_10_7_x86_64.whl

echo "Check wheel"
delocate-listdeps target/wheels/iterframes-*-abi3-macosx_10_7_x86_64.whl
