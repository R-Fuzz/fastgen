mkdir -p outseeds
cat /src/cgc_list | while read line; do
mkdir -p ${line}_outseeds
{ time SYMCC_INPUT_FILE=/out/cgc_seeds/$line/seed cat /out/cgc_seeds/$line/seed | SYMCC_OUTPUT_DIR=${line}_outseeds SYMCC_ENABLE_LINEARIZATION=1 timeout -k 10 300 ./challenges/$line/$line  1>/dev/null 2>/dev/null; } 2>&1 | grep real
done
