#ifndef UTIL_H_
#define UTIL_H_
using namespace rgd;
bool saveRequest(const google::protobuf::MessageLite& message,
								 const char* path);
void printNode(const AstNode* node);
void printNode(const RealAstNode* node);
void printTask(const SearchTask* task);
#endif
