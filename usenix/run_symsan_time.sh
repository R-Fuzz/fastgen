 dir=$1
 pro=$2
 opt=$3
 find ${dir} -name "id*" | while read line; do
    { time  ( TAINT_OPTIONS="taint_file=$line" ./${pro}.symsan ${opt} $line &> /dev/null ; )   }  2>&1  | grep real
 done
