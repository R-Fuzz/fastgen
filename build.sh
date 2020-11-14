cd searcher
mkdir -p build
cd build
cmake .. && make -j
cd ../../
cargo build
cargo run
