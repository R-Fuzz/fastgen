mkdir -p outseeds
cat /src/cgc_list | while read line; do
{ time cat /out/cgc_seeds/$line/seed | SYMCC_INPUT_FILE=/out/cgc_seeds/$line/seed SYMCC_OUTPUT_DIR=outseeds SYMCC_ENABLE_LINEARIZATION=1 timeout -k 10 300 /symqemu/build/x86_64-linux-user/symqemu-x86_64 ./challenges/$line/$line  1>/dev/null 2>/dev/null; } 2>&1 | grep real
done
