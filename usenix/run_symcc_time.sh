dir=$1 
pro=$2
opt=$3
mkdir -p outseeds
find ${dir} -name "id*" | while read line; do
  { time SYMCC_INPUT_FILE=$line SYMCC_ENABLE_LINEARIZATION=1 SYMCC_OUTPUT_DIR=outseeds ./${pro}.symcc ${opt} ${line} 1>/dev/null 2>/dev/null; } 2>&1 | grep real
done
