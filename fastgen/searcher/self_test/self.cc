//#include "help.h"
#include <google/protobuf/io/zero_copy_stream_impl.h>
#include <google/protobuf/io/coded_stream.h>
#include <sys/types.h>
#include <sys/stat.h>
#include <fcntl.h>
#include "rgd.pb.h"
#include "util.h"
#include "parser.h"
#include "task.h"
#include "interface.h"
#include "gd.h"

using namespace google::protobuf::io;

using namespace rgd;


int main() {
  init_searcher();
  int fd = open("../test.data",O_RDONLY);
  ZeroCopyInputStream* rawInput = new google::protobuf::io::FileInputStream(fd);
  bool suc = false;
  int fid = 1;
  do {
    SearchTask task;
    suc = readDelimitedFrom(rawInput,&task);
    if (suc) {
//      printTask(&task);
      FUT* fut = construct_task(&task);
      std::unordered_map<uint32_t, uint8_t> rgd_solution;
      fut->rgd_solution = &rgd_solution;
      gd_search(fut); 
      generate_input(rgd_solution, "/home/cju/test/i", "/home/cju/test", fid++);
    }
  } while (suc);
}

