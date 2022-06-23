 dir=$1
 pro=$2
 opt=$3
 find ${dir} -name "id*" | while read line; do
    { time  ( ./${pro}.native ${opt} $line &> /dev/null ; )   }  2>&1  | grep real
 done
