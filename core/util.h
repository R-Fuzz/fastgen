#ifndef UTIL_H_
#define UTIL_H_
using namespace rgd;
bool saveRequest(const google::protobuf::MessageLite& message,
								 const char* path);
void printExpression(const JitRequest* req);
#endif
