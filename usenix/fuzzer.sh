num_seeds=$1
pro=$2
opt=$3
RUST_LOG=info ./fastgen  -i /out/real_seeds/${pro}_reduced/ -s ${num_seeds} -o corpus_${pro} -t ./${pro}.symsan -- ./${pro}.symsan ${opt} @@
