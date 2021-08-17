#!/bin/bash
rm -rf corpus
#RUST_LOG=info ../target/release/fastgen --sync_afl -i seeds -o corpus -t ./standard_fuzzer_kir -- ./standard_fuzzer_fast @@
RUST_BACKTRACE=1 RUST_LOG=info ../target/release/fastgen --sync_afl -i seeds -o corpus -t ./objdump.tracko1newpipe -- ./objdump.fast -D @@
#RUST_BACKTRACE=1 RUST_LOG=info ../../workdir/fastgen/target/release/fastgen --sync_afl -i seeds -o corpus -t ./objdump.tracko1newpipe -- ./objdump.fastold -D @@

