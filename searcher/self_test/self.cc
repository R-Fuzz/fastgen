//#include "help.h"
#include <google/protobuf/io/zero_copy_stream_impl.h>
#include <google/protobuf/io/coded_stream.h>
#include <sys/types.h>
#include <sys/stat.h>
#include <fcntl.h>
#include "rgd.pb.h"
#include "util.h"
#include "parser.h"
#include "interface.h"

using namespace google::protobuf::io;

using namespace rgd;


int main() {
  init_searcher();
  int fd = open("../test.data",O_RDONLY);
  ZeroCopyInputStream* rawInput = new google::protobuf::io::FileInputStream(fd);
  SearchTask task;
  readDelimitedFrom(rawInput,&task);
  printTask(&task);
  construct_task(&task);
}
