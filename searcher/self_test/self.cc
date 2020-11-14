//#include "help.h"
#include <google/protobuf/io/zero_copy_stream_impl.h>
#include <google/protobuf/io/coded_stream.h>
#include <sys/types.h>
#include <sys/stat.h>
#include <fcntl.h>
#include "rgd.pb.h"
#include "util.h"

using namespace google::protobuf::io;

using namespace rgd;


int main() {
  int fd = open("../test.data",O_RDONLY);
  printf("fd is %d\n", fd);
  ZeroCopyInputStream* rawInput = new google::protobuf::io::FileInputStream(fd);
  SearchTask task;
  readDelimitedFrom(rawInput,&task);
  //task.ParseFromCodedStream(&s);
  printf("task construct success\n");
  printTask(&task);
}
