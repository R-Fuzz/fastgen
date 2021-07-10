#ifndef UTIL_H_
#define UTIL_H_
//using namespace rgd;
//bool saveRequest(const google::protobuf::MessageLite& message,
//								 const char* path);
//void printNode(const AstNode* node);
//void printTask(const SearchTask* task);

//ool readDelimitedFrom(
//		google::protobuf::io::ZeroCopyInputStream* rawInput,
//		google::protobuf::MessageLite* message);

uint64_t getTimeStamp();
void generate_input(std::unordered_map<uint32_t,uint8_t> &sol, std::string taint_file, std::string outputDir, uint32_t fid);
uint32_t load_input(std::string taint_file, unsigned char* input);
#endif
