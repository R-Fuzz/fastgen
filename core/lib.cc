#include <stdio.h>
#include <google/protobuf/io/coded_stream.h>
#include "rgd.pb.h"
#include "util.h"
using namespace rgd;
using namespace google::protobuf::io;


void parse_bytes(const unsigned char* input, unsigned int input_length) {
  CodedInputStream s(input,input_length);
  s.SetRecursionLimit(10000);
  JitCmdv2 cmd;
  cmd.ParseFromCodedStream(&s);
  printExpression(&cmd.expr(0));
}

extern "C" {
  void print_buffer(const unsigned char* input, unsigned int input_length) {
    for (int i=0; i<input_length;i++)
      printf("%d,",input[i]);
    printf("\n");
    parse_bytes(input,input_length);
  }
};

