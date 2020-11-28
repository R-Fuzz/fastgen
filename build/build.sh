#!/bin/bash
BIN_PATH=$(readlink -f "$0")
ROOT_DIR=$(dirname $(dirname $BIN_PATH))

set -euxo pipefail

PREFIX1=${PREFIX:-${ROOT_DIR}/bin/}
PREFIX2=${PREFIX:-${ROOT_DIR}/bin_ang/}

cd fuzzer/cpp_core
mkdir -p build
cd build
cmake .. && make -j
cd ../../..

cargo build
cargo build --release

rm -rf ${PREFIX2}
mkdir -p ${PREFIX2}
mkdir -p ${PREFIX2}/lib
#cp target/release/fuzzer ${PREFIX2}
cp target/release/*.a ${PREFIX2}/lib


cd llvm_mode
rm -rf build
mkdir -p build
cd build
cmake -DCMAKE_INSTALL_PREFIX=${PREFIX1} -DCMAKE_BUILD_TYPE=Release ..
make -j
make install
cd ../../

cd llvm_mode_angora
mkdir -p build
cd build
cmake -DCMAKE_INSTALL_PREFIX=${PREFIX2} -DCMAKE_BUILD_TYPE=Release ..
make -j
make install

