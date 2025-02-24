#!/bin/bash

# This script is used to build multiple wheels, with the manylinux docker container

set -ex

curl https://sh.rustup.rs -sSf | sh -s -- --default-toolchain stable -y
export PATH="$HOME/.cargo/bin:$PATH"
# libclang is in non standard path
export LIBCLANG_PATH=/opt/rh/llvm-toolset-7.0/root/usr/lib64
export LD_LIBRARY_PATH=$LIBCLANG_PATH:$LD_LIBRARY_PATH
# c stdlib headers
export BINDGEN_EXTRA_CLANG_ARGS="-I/opt/rh/devtoolset-10/root/usr/lib/gcc/x86_64-redhat-linux/10/include -I/usr/include"


# map libmicrovmi root dir to /io
cd /io/python

# note: removed 3.5
for PYBIN in /opt/python/cp{36,37,38,39}*/bin; do
    "${PYBIN}/pip" install -r requirements.txt
    "${PYBIN}/python" setup.py bdist_wheel $@
done

for whl in dist/*.whl; do
    auditwheel repair "$whl" -w dist/manylinux
done
