dir=$1 
pro=$2
opt=$3
find ${dir} -name "id*" | while read line; do
  SYMCC_INPUT_FILE=$line SYMCC_ENABLE_LINEARIZATION=1 SYMCC_OUTPUT_DIR=xx  ./${pro}.symcc ${opt} $line 2>/dev/null 1>/dev/null
done
