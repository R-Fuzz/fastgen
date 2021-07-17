#!/bin/bash
rm -rf corpus
#RUST_LOG=info ../target/release/fastgen --sync_afl -i seeds -o corpus -t ./standard_fuzzer_kir -- ./standard_fuzzer_fast @@
RUST_BACKTRACE=1 RUST_LOG=info ../target/release/fastgen --sync_afl -i seeds -o corpus -t ./libjpeg_turbo_fuzzer_kir_new -- ./libjpeg_turbo_fuzzer_fast_new @@

