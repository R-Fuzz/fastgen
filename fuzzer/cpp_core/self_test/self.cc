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
  do {
    std::shared_ptr<SearchTask>  task = std::make_shared<SearchTask>();
    suc = readDelimitedFrom(rawInput,task.get());
    printf("run here\n");
    bool solve = true;
    std::pair<std::shared_ptr<SearchTask>,bool> tt{task,solve};
    task_queue.push(tt);
  } while (suc);
  printf("returning\n");
  while (true) {}
  return 0;
}

