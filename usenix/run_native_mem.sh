dir=$1 
pro=$2
opt=$3
find ${dir} -name "id*" | while read line; do
  /usr/bin/time -a -o symsan_mem_log -f '%E %M' ./${pro}.native ${opt} $line 1>/dev/null 2>/dev/null
done
