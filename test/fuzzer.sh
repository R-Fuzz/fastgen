#!/bin/bash
rm -rf corpus
RUST_LOG=info ../target/release/fastgen --sync_afl -i input_nm -o corpus -t ./nm.tracko1 -- ./nm.fastsock -C @@

