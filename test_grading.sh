rm -rf fuzzer/output
RUST_MIN_STACK=8388608 RUST_LOG=info cargo test test_grading -- --nocapture
