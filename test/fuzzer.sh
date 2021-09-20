#!/bin/bash
rm -rf corpus
RUST_LOG=info ../target/release/fastgen --sync_afl -i input_tiff -o corpus -t ./tiff.track -- ./tiff.fast @@

