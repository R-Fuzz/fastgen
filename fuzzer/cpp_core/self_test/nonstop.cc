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
#include "ctpl.h"


using namespace google::protobuf::io;

using namespace rgd;
extern TaskQueue task_queue;

int main() {
  init(false, true);
  int fd = open("../regression.data",O_RDONLY);
  ZeroCopyInputStream* rawInput = new google::protobuf::io::FileInputStream(fd);
  bool suc = false;
  int fid = 1;
  int finished = 0;
  std::shared_ptr<SearchTask>  task = std::make_shared<SearchTask>();
  suc = readDelimitedFrom(rawInput,task.get());
  //printTask(task.get());
  if (suc) {
    for (int i=0 ;i<100; i++) {
      printf("#%d search\n", i);
      //handle_task(0,task);
    }
  }
  printf("out of loop\n");
  delete rawInput;
  printf("returning \n");
  return 0;
}

