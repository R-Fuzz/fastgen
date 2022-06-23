dir=$1 
pro=$2
opt=$3
find ${dir} -name "id*" | while read line; do
  TAINT_OPTIONS=taint_file=$line /usr/bin/time -a -o symsan_mem_log -f '%E %M' ./${pro}.symsan ${opt} $line 1>/dev/null 2>/dev/null
done
