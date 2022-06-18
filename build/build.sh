#!/bin/bash
BIN_PATH=$(readlink -f "$0")
ROOT_DIR=$(dirname $(dirname $BIN_PATH))

set -euxo pipefail

PREFIX=${PREFIX:-${ROOT_DIR}/bin/}

unset CXXFLAGS
unset CFLAGS
cd fuzzer/cpp_core
rm -rf build
mkdir -p build
cd build
cmake .. && make -j
cd ../../..

cargo build
cargo build --release

rm -rf ${PREFIX}
mkdir -p ${PREFIX}
mkdir -p ${PREFIX}/lib
cp target/release/*.a ${PREFIX}/lib


pushd llvm_mode
rm -rf build
mkdir -p build
pushd build
CC=clang-6.0 CXX=clang++-6.0 cmake -DCMAKE_INSTALL_PREFIX=${PREFIX} -DCMAKE_BUILD_TYPE=Release ..
make -j
make install
popd
popd
