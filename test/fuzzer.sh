#!/bin/bash
rm -rf corpus
RUST_BACKTRACE=full RUST_LOG=info ../target/release/fastgen --sync_afl -i input_openssl -o corpus -t ./x509.track -- ./x509.fast @@
#RUST_BACKTRACE=1 RUST_LOG=info ../../workdir/fastgen/target/release/fastgen --sync_afl -i seeds -o corpus -t ./objdump.tracko1newpipe -- ./objdump.fastold -D @@

