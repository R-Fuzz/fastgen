rm -rf  corpus
#HEAPCHECK=normal LD_PRELOAD="/usr/local/lib/libtcmalloc.so" RUST_LOG=info ../target/debug/fastgen -i input -o output -t ./objdump.track -- ./objdump.fast -D @@
LD_PRELOAD="/usr/local/lib/libtcmalloc.so" RUST_LOG=info ../target/release/fastgen --sync_afl -i input -o corpus -t ./objdump.nofilter -- ./objdump.fast -D @@
