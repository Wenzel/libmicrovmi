#!/bin/bash

# This script is used to build multiple wheels, with the manylinux docker container

set -ex

curl https://sh.rustup.rs -sSf | sh -s -- --default-toolchain stable -y
export PATH="$HOME/.cargo/bin:$PATH"

# map libmicrovmi root dir to /io
cd /io/python

# note: removed 3.5
for PYBIN in /opt/python/cp{36,37,38,39}*/bin; do
    "${PYBIN}/pip" install -U setuptools wheel setuptools-rust
    "${PYBIN}/python" setup.py bdist_wheel
done

for whl in dist/*.whl; do
    auditwheel repair "$whl" -w dist/
done
