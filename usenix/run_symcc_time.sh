dir=$1 
pro=$2
opt=$3
find ${dir} -name "id*" | while read line; do
  { time SYMCC_OUTPUT_DIR=xx SYMCC_NO_SYMBOLIC_INPUT=1 ./${pro}.symcc ${opt} ${line} 1>/dev/null 2>/dev/null; } 2>&1 | grep real
done
