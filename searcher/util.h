#ifndef UTIL_H_
#define UTIL_H_
using namespace rgd;
bool saveRequest(const google::protobuf::MessageLite& message,
								 const char* path);
void printNode(const AstNode* node);
void printTask(const SearchTask* task);

bool readDelimitedFrom(
		google::protobuf::io::ZeroCopyInputStream* rawInput,
		google::protobuf::MessageLite* message);
#endif
