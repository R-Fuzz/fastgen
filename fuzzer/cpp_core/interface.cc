#include "llvm/ADT/APFloat.h"
#include "llvm/ADT/STLExtras.h"
#include "llvm/IR/BasicBlock.h"
#include "llvm/IR/Constants.h"
#include "llvm/IR/DerivedTypes.h"
#include "llvm/IR/Function.h"
#include "llvm/IR/IRBuilder.h"
#include "llvm/IR/LLVMContext.h"
#include "llvm/IR/LegacyPassManager.h"
#include "llvm/IR/Module.h"
#include "llvm/IR/Type.h"
#include "llvm/IR/Verifier.h"
#include "llvm/Support/TargetSelect.h"
#include "llvm/Target/TargetMachine.h" 
#include "llvm/Transforms/InstCombine/InstCombine.h" 
#include "llvm/Transforms/Scalar.h"
#include "llvm/Transforms/Scalar/GVN.h"
#include <stdio.h>
#include <google/protobuf/io/coded_stream.h>
#include "rgd.pb.h"
#include "util.h"
#include "rgdJit.h"
#include "gd.h"
#include "task.h"
#include "parser.h"
#include "ctpl.h"
#include "queue.h"
using namespace rgd;
using namespace google::protobuf::io;

#define THREAD_POOL_SIZE 32

//global variables
std::unique_ptr<GradJit> JIT;
static std::atomic<uint64_t> fid;
ctpl::thread_pool* pool;
bool SAVING_WHOLE; 
bool USE_CODECACHE;
moodycamel::ConcurrentQueue<std::pair<uint32_t, std::unordered_map<uint32_t,uint8_t>>> solution_queue;
std::vector<std::future<bool>> gresults;

void save_task(const unsigned char* input, unsigned int input_length) {
  CodedInputStream s(input,input_length);
  s.SetRecursionLimit(10000);
  SearchTask task;
  task.ParseFromCodedStream(&s);
  printTask(&task);
  saveRequest(task, "test.data");
}

bool handle_task(int tid, std::shared_ptr<SearchTask> task) {
  FUT* fut = construct_task(task.get());
  std::unordered_map<uint32_t, uint8_t> rgd_solution;
  fut->rgd_solution = &rgd_solution;
  gd_search(fut);
  if (rgd_solution.size() == 0) {
    printf("not solved\n");
    return false;
  }
  if (!SAVING_WHOLE) {
    solution_queue.enqueue({task->fid(), rgd_solution});
    if (solution_queue.size_approx() % 1000 == 0)
     printf("queue item is about %u\n", solution_queue.size_approx());
  } else {
    std::string old_string = std::to_string(task->fid());
    std::string input_file = "/home/cju/fastgen/test/output/queue/id:" + std::string(6-old_string.size(),'0') + old_string;
    for(auto itr : rgd_solution)
        printf("sol index is %u and value is %u\n",itr.first,itr.second);
    generate_input(rgd_solution, input_file, "/home/cju/test", fid++);
  }
  delete fut;
  return true;
}

void init(bool saving_whole, bool use_codecache) {
  llvm::InitializeNativeTarget();
  llvm::InitializeNativeTargetAsmPrinter();
  llvm::InitializeNativeTargetAsmParser();
  JIT = std::move(GradJit::Create().get());
  pool = new ctpl::thread_pool(THREAD_POOL_SIZE,0);
  SAVING_WHOLE = saving_whole;
  USE_CODECACHE = use_codecache;
}

std::string get_current_dir() {
   char buff[FILENAME_MAX]; //create string buffer to hold path
   getcwd( buff, FILENAME_MAX );
   std::string current_working_dir(buff);
   return current_working_dir;
}

extern "C" {
  void submit_task(const unsigned char* input, unsigned int input_length) {
    CodedInputStream s(input,input_length);
    s.SetRecursionLimit(10000);
    std::shared_ptr<SearchTask> task = std::make_shared<SearchTask>();
    task->ParseFromCodedStream(&s);
    printTask(task.get());
    //gresults.emplace_back(pool->push(handle_task, task));
    handle_task(0,task);
  }

  void init_core(bool saving_whole, bool use_codecache) { init(saving_whole, use_codecache); }
  void aggregate_results() {
    int finished = 0;
    for(auto && r: gresults) {
      finished += (int)r.get();
    } 
  }
  
  void get_input_buf(unsigned char* input) {
    for(int i=0; i<10;i++) {
      input[i] = 32 ;
    }
  }

  uint32_t get_next_input(unsigned char* input) {
    std::pair<uint32_t, std::unordered_map<uint32_t, uint8_t>> item;
 //   printf("get_next_loop and queue size is %u\n", solution_queue.size_approx());
    if(solution_queue.try_dequeue(item)) {
      std::string old_string = std::to_string(item.first);
      std::string input_file = "output/queue/id:" + std::string(6-old_string.size(),'0') + old_string;
      uint32_t size = load_input(input_file, input);
      for(auto it = item.second.begin(); it != item.second.end(); ++it)
        input[it->first] = it->second;
      return size;
    } else {
      return 0; 
    }
  }
};

