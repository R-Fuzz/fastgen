#ifndef UTIL_H_
#define UTIL_H_
uint64_t getTimeStamp();
void generate_input(std::unordered_map<uint32_t,uint8_t> &sol, std::string taint_file, std::string outputDir, uint32_t fid);
uint32_t load_input(std::string taint_file, unsigned char* input);
#endif
