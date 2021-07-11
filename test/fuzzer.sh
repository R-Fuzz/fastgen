#!/bin/bash
rm -rf corpus
RUST_LOG=info ../target/release/fastgen --sync_afl -i jpeg_seeds -o corpus -t ./libjpeg_turbo_fuzzer_kir -- ./libjpeg_turbo_fuzzer_fast @@

