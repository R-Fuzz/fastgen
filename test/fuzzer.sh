#!/bin/bash
rm -rf corpus
RUST_LOG=info ../target/release/fastgen --sync_afl -i proj_seeds -o corpus -t ./standard_fuzzer.track -- ./standard_fuzzer.fast @@

