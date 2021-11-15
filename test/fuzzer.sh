#!/bin/bash
pro=$1
rm -rf corpus
RUST_BACKTRACE=full RUST_LOG=info ../target/release/fastgen --sync_afl -i input_${pro} -o corpus -t ./${pro}.track -- ./${pro}.fast @@
#RUST_BACKTRACE=1 RUST_LOG=info ../../workdir/fastgen/target/release/fastgen --sync_afl -i seeds -o corpus -t ./${pro}.tracko1newpipe -- ./${pro}.fastold -D @@

