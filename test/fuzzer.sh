#!/bin/bash
rm -rf corpus
RUST_BACKTRACE=full RUST_LOG=info ../target/release/fastgen --sync_afl -i input_libjpeg -o corpus -t ./libjpeg_turbo_fuzzer.track -- ./libjpeg_turbo_fuzzer.fast @@
#RUST_BACKTRACE=1 RUST_LOG=info ../../workdir/fastgen/target/release/fastgen --sync_afl -i seeds -o corpus -t ./objdump.tracko1newpipe -- ./objdump.fastold -D @@

