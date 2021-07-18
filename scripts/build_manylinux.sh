#!/usr/bin/env bash
set -x
set -e

DOCKER_IMAGE="iterframes_manylinux_2010"

echo "Build docker image"
docker build . -t $DOCKER_IMAGE

echo "Compile project"
docker run --rm -it -v $(pwd):/io $DOCKER_IMAGE build --release --strip --manylinux off

echo "Run auditwheel repair"
docker run --rm -it -v $(pwd):/io --entrypoint /usr/bin/env $DOCKER_IMAGE auditwheel repair target/wheels/iterframes-*-linux_x86_64.whl
