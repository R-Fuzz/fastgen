cat /src/cgc_list | while read line; do
{ time timeout -k 10 300 ./challenges/$line/$line < /out/cgc_seeds/$line/seed 1>/dev/null 2>/dev/null; } 2>&1 | grep real
done
