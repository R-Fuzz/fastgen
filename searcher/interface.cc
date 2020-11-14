#include <stdio.h>
#include <google/protobuf/io/coded_stream.h>
#include "rgd.pb.h"
#include "util.h"
using namespace rgd;
using namespace google::protobuf::io;


void parse_bytes(const unsigned char* input, unsigned int input_length) {
  CodedInputStream s(input,input_length);
  s.SetRecursionLimit(10000);
  SearchTask task;
  task.ParseFromCodedStream(&s);
  printTask(&task);
  saveRequest(task, "test.data");
}

extern "C" {
  void print_buffer(const unsigned char* input, unsigned int input_length) {
    parse_bytes(input,input_length);
  }
};

