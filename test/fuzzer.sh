#!/bin/bash
mkfifo /tmp/wp2
mkfifo /tmp/wp3
rm -rf corpus
RUST_LOG=info ../target/release/fastgen --sync_afl -i seeds -o corpus -t ./standard_fuzzer_kir -- ./standard_fuzzer_fast @@

