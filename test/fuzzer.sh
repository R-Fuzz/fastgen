rm -rf output
redis-cli flushall
#HEAPCHECK=normal LD_PRELOAD="/usr/local/lib/libtcmalloc.so" RUST_LOG=info ../target/debug/fastgen -i input -o output -t ./objdump.track -- ./objdump.fast -D @@
LD_PRELOAD="/usr/local/lib/libtcmalloc.so" RUST_LOG=info ../target/release/fastgen -i input -o output -t ./objdump.order -- ./objdump.fast -D @@
