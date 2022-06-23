dir=$1 
pro=$2
opt=$3
mkdir -p outseeds
find ${dir} -name "id*" | while read line; do
  SYMCC_INPUT_FILE=$line SYMCC_ENABLE_LINEARIZATION=1 SYMCC_OUTPUT_DIR=outseeds /usr/bin/time -a -o symcc_mem_log -f '%E %M' ./${pro}.symcc ${opt} $line 1>/dev/null 2>/dev/null
done
