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
#include "pcqueue.h"
using namespace rgd;
using namespace google::protobuf::io;

#define THREAD_POOL_SIZE 1
#define DEBUG 1
//global variables
std::unique_ptr<GradJit> JIT;
static std::atomic<uint64_t> fid;
static std::atomic<uint64_t> task_id;
bool SAVING_WHOLE; 
bool USE_CODECACHE;
bool sendZ3Solver(bool opti, SearchTask* task, std::unordered_map<uint32_t, uint8_t> &solu, uint64_t addr, bool solve);
void initZ3Solver();
void addCons(SearchTask* task);
//moodycamel::ConcurrentQueue<std::pair<uint32_t, std::unordered_map<uint32_t,uint8_t>>> solution_queue;
std::vector<std::future<bool>> gresults;

struct RGDSolution {
    std::unordered_map<uint32_t, uint8_t> sol;
  //the intended branch for this solution
    uint32_t fid;  //the seed
    uint64_t addr;
    uint64_t ctx;
    uint32_t order;
    uint64_t direction;
};

moodycamel::ConcurrentQueue<RGDSolution> solution_queue;
moodycamel::ConcurrentQueue<RGDSolution> higher_solution_queue;
//moodycamel::ConcurrentQueue<std::pair<std::shared_ptr<SearchTask>, bool>> incoming_tasks(10000000);
folly::ProducerConsumerQueue<std::pair<std::shared_ptr<SearchTask>, bool>> incoming_tasks1(10000000);
folly::ProducerConsumerQueue<std::pair<std::shared_ptr<SearchTask>, bool>> incoming_tasks_higher(10000000);

void save_task(const unsigned char* input, unsigned int input_length) {
  CodedInputStream s(input,input_length);
  s.SetRecursionLimit(10000);
  SearchTask task;
  task.ParseFromCodedStream(&s);
  printTask(&task);
  saveRequest(task, "regression.data");
}

//bool handle_task(int tid, std::shared_ptr<SearchTask> task) {
void* handle_task(void*) {
  //printTask(task.get());
  while (1) {
    std::pair<std::shared_ptr<SearchTask>, bool> task1;
    //if (incoming_tasks.try_dequeue(task1)) {
    // let's try higher order first
    if (incoming_tasks_higher.isEmpty()) {
        continue;
    } else {
      incoming_tasks_higher.read(task1);
    }
    std::shared_ptr<SearchTask> task = task1.first;
    bool solve = task1.second;

    FUT* fut = nullptr;
    FUT* fut_opt = nullptr;

    bool n_solvable = false;
    bool s_solvable = false;
    bool z3n_solvable = false;
    bool z3s_solvable = false;

    lookup_or_construct(task.get(), &fut, &fut_opt, task1.second);

    std::vector<std::unordered_map<uint32_t, uint8_t>> rgd_solutions;
    std::vector<std::unordered_map<uint32_t, uint8_t>> partial_solutions;
    std::vector<std::unordered_map<uint32_t, uint8_t>> rgd_solutions_opt;
    std::unordered_map<uint32_t, uint8_t> z3_solution;
    fut->rgd_solutions = &rgd_solutions;
    fut->partial_solutions = &partial_solutions;
    fut_opt->rgd_solutions = &rgd_solutions_opt;
    
#if 0
    gd_search(fut_opt);
    if (rgd_solutions_opt.size() != 0) {
      s_solvable = true;
      //fut->load_hint(rgd_solutions_opt[0]);
      gd_search(fut);
    } else {
      s_solvable = false;
    }
#endif
#if 0
    fut_opt->flip();
    fut->flip();
    gd_search(fut_opt);
    if (rgd_solutions_opt.size() != 0) {
      s_solvable = true;
      //fut->load_hint(rgd_solutions_opt[0]);
      gd_search(fut);
    } else {
      s_solvable = false;
    }
    fut_opt->flip();
    fut->flip();
#endif

#if 1
    //if (rgd_solutions.size() == 0) {
    bool ret = sendZ3Solver(false, task.get(), z3_solution, task->addr(), solve);
    if (!ret)
      sendZ3Solver(true, task.get(), z3_solution, task->addr(), solve);
    //}
#endif


    if (!SAVING_WHOLE) {
      for (auto rgd_solution :  rgd_solutions) {
        RGDSolution sol = {rgd_solution, task->fid(), task->addr(), task->ctx(), task->order(), task->direction()};
        higher_solution_queue.enqueue(sol);
#if DEBUG
        //if (solution_queue.size_approx() % 1000 == 0)
          //printf("queue item is about %u\n", solution_queue.size_approx());
#endif
      }
/*
      for (auto rgd_solution :  rgd_solutions_opt) {
        RGDSolution sol = {rgd_solution, task->fid(), task->addr(), task->ctx(), task->order(), task->direction()};
        if (fresh)
        higher_solution_queue.enqueue(sol);
	else
        solution_queue.enqueue(sol);
#if DEBUG
        //if (solution_queue.size_approx() % 1000 == 0)
          //printf("queue item is about %u\n", solution_queue.size_approx());
#endif
      }
*/
      for (auto rgd_solution :  partial_solutions) {
        RGDSolution sol = {rgd_solution, task->fid(), task->addr(), task->ctx(), task->order(), task->direction()};
        solution_queue.enqueue(sol);
#if DEBUG
        if (solution_queue.size_approx() % 1000 == 0)
          printf("queue item is about %u\n", solution_queue.size_approx());
#endif
      }

      if (z3_solution.size() != 0) {
        RGDSolution sol = {z3_solution, task->fid(), task->addr(), task->ctx(), task->order(), task->direction()};
        solution_queue.enqueue(sol);
      }

    } else {
      // std::string old_string = std::to_string(task->fid());
      std::string input_file = "/home/cju/fastgen/test/seed";
      // std::string input_file = "/home/cju/fastgen/tests/switch/input_switch/i";
      //std::string input_file = "corpus/angora/queue/id:" + std::string(6-old_string.size(),'0') + old_string;
      for (auto rgd_solution : rgd_solutions) {
        generate_input(rgd_solution, input_file, "/home/cju/test", fid++);
      }
      for (auto rgd_solution : rgd_solutions_opt) {
        generate_input(rgd_solution, input_file, "/home/cju/test", fid++);
      }
      for (auto rgd_solution : partial_solutions) {
        generate_input(rgd_solution, input_file, "/home/cju/test", fid++);
      }

      if (z3_solution.size() != 0)
        generate_input(z3_solution, input_file, "/home/cju/test", fid++);

    }

    //delete fut;
    //delete fut_opt;
    //return n_solvable || s_solvable || z3n_solvable || z3s_solvable ;
  }
  return nullptr;
  }

void init(bool saving_whole, bool use_codecache) {
  llvm::InitializeNativeTarget();
  llvm::InitializeNativeTargetAsmPrinter();
  llvm::InitializeNativeTargetAsmParser();
  JIT = std::move(GradJit::Create().get());
  //pool = new ctpl::thread_pool(THREAD_POOL_SIZE,0);
  pthread_t thread;
  pthread_attr_t attr;
  pthread_attr_init(&attr);
  pthread_attr_setdetachstate(&attr, PTHREAD_CREATE_JOINABLE);
  pthread_create(&thread, &attr, handle_task, nullptr);
  SAVING_WHOLE = saving_whole;
  USE_CODECACHE = use_codecache;
  initZ3Solver();
}


void fini() {
  //delete pool;
  //pthread_join();
}

std::string get_current_dir() {
   char buff[FILENAME_MAX]; //create string buffer to hold path
   getcwd( buff, FILENAME_MAX );
   std::string current_working_dir(buff);
   return current_working_dir;
}

void handle_fmemcmp(uint8_t* data, uint64_t index, uint32_t size, uint32_t tid, uint64_t addr) {
  std::unordered_map<uint32_t, uint8_t> rgd_solution;
  std::string input_file = "/home/cju/fastgen/test/seed";
 // std::string old_string = std::to_string(tid);
  //std::string input_file = "/home/cju/fastgen/tests/switch/input_switch/i";
  //std::string input_file = "corpus/angora/queue/id:" + std::string(6-old_string.size(),'0') + old_string;
  for(uint32_t i=0;i<size;i++) {
    //rgd_solution[(uint32_t)index+i] = (uint8_t) (data & 0xff);
    rgd_solution[(uint32_t)index+i] = data[i];
    //data = data >> 8 ;
  }
  if (SAVING_WHOLE) {
    generate_input(rgd_solution, input_file, "/home/cju/test", fid++);
  }
  else {
    RGDSolution sol = {rgd_solution, tid, addr, 0, 0};
    solution_queue.enqueue(sol);
  }
}

extern "C" {
  void submit_fmemcmp(uint8_t* data, uint64_t index, uint32_t size, uint32_t tid, uint64_t addr) {
      //RGDSolution sol = {rgd_solution, 0, 0, 0, 0};
      handle_fmemcmp(data,index,size, tid, addr);
  }


  uint32_t get_queue_length() {
    return incoming_tasks1.sizeGuess();
  }

  void submit_task(const unsigned char* input, unsigned int input_length, bool expect_future, bool solve) {
    CodedInputStream s(input,input_length);
    s.SetRecursionLimit(10000);
    std::shared_ptr<SearchTask> task = std::make_shared<SearchTask>();
    task->ParseFromCodedStream(&s);
    //printTask(task.get());
/*
    if (expect_future)
      gresults.emplace_back(pool->push(handle_task, task));
    else
      pool->push(handle_task, task);
*/

//    handle_task(0,task);
    //incoming_tasks.enqueue({task, fresh});
    std::pair<std::shared_ptr<SearchTask>,bool> tt{task,solve};
    incoming_tasks_higher.write(tt);
        //if (incoming_tasks1.size_approx() % 1000 == 0)
    if (incoming_tasks_higher.sizeGuess() % 1000 == 0 && incoming_tasks_higher.sizeGuess() > 0)
      printf("queue tasks is about %u\n", incoming_tasks1.sizeGuess());
  }

  void init_core(bool saving_whole, bool use_codecache) { init(saving_whole, use_codecache); }
  void fini_core() { fini(); }
  void aggregate_results() {
    int finished = 0;
    for(auto && r: gresults) {
      finished += (int)r.get();
    } 
  }
  
  void get_input_buf(unsigned char* input) {
    for(int i=0; i<10;i++) {
      input[i] = 32;
    }
  }

  uint32_t get_next_input(unsigned char* input, uint64_t *addr, uint64_t *ctx, 
          uint32_t *order, uint32_t *fid, uint64_t *direction) {
    //std::pair<uint32_t, std::unordered_map<uint32_t, uint8_t>> item;
    RGDSolution item;
    //if (solution_queue.size_approx() % 1000 == 0 && solution_queue.size_approx() > 0)
     // printf("get_next_loop and queue size is %u\n", solution_queue.size_approx());
    if(higher_solution_queue.try_dequeue(item)) {
      std::string old_string = std::to_string(item.fid);
      std::string input_file = "corpus/angora/queue/id:" + std::string(6-old_string.size(),'0') + old_string;
      //std::string input_file = "/home/cju/debug/seed.png";
      uint32_t size = load_input(input_file, input);
      for(auto it = item.sol.begin(); it != item.sol.end(); ++it)
        input[it->first] = it->second;
      *addr = item.addr;
      *ctx = item.ctx;
      *order = item.order;
      *fid = item.fid;
      *direction = item.direction;
      return size;
    } else if (solution_queue.try_dequeue(item)) {
      //smapling output
      //uint32_t random_fid = get_random_fid(item.addr, item.ctx, item.order, item.direction);
      //if (random_fid == -1) random_fid = item.fid;
      uint32_t random_fid = item.fid;
      std::string old_string = std::to_string(random_fid);
      std::string input_file = "corpus/angora/queue/id:" + std::string(6-old_string.size(),'0') + old_string;
      //std::string input_file = "/home/cju/debug/seed.png";
      uint32_t size = load_input(input_file, input);
      for(auto it = item.sol.begin(); it != item.sol.end(); ++it) {
        if (it->first < size)
          input[it->first] = it->second;
      }
      *addr = item.addr;
      *ctx = item.ctx;
      *order = item.order;
      *fid = item.fid;
      *direction = item.direction;
      return size;
    } else {
      return 0; 
    }
  }
};

