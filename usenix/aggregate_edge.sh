dir=$1
pro=$2
opt=$3
#rm -rf OUTPUT
rm -rf parse_result*

if [ -f "parse_to_be" ]; then
    echo "parse_to_be exists, remove it!" 
    rm parse_to_be
fi

count=0
find ${dir} -maxdepth 1  -name "id\:*" -printf "%T@ %Tc %p\n"  | sort -n  | cut -d " " -f 9 | while read line; do
		echo $line
    stat -c '%y' $line |  grep -a "2021" >> parse_result_${pro}
    filename=$(basename -- "$line")
		ASAN_OPTIONS=coverage=1 ./${pro}_cov ${opt}  $line 1>/dev/null 2>/dev/null
   
		ls -rt *.sancov | while read line1; do
			if  test -f OUTPUT; then
				./sancov.py merge OUTPUT $line1  1> OUTPUT_NEW 2>>parse_result_${pro}
			else
				./sancov.py merge $line1  1> OUTPUT_NEW 2>>parse_result_${pro}
			fi
			mv OUTPUT_NEW OUTPUT
			unlink $line1
		done
done
