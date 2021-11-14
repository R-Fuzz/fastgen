#!/bin/bash
rm -rf corpus
RUST_LOG=info ../target/release/fastgen --sync_afl -i input_openssl -o corpus -t ./openssl.track -- ./openssl.fast @@

