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
#include <condition_variable>
#include <google/protobuf/io/coded_stream.h>
#include "rgd.pb.h"
#include "util.h"
#include "rgdJit.h"
#include "gd.h"
#include "task.h"
#include "parser.h"
#include "interface.h"
using namespace rgd;
using namespace google::protobuf::io;

#define THREAD_POOL_SIZE 1
#define DEBUG 1

uint64_t getTimeStamp();
//global variables
std::unique_ptr<GradJit> JIT;
static std::atomic<uint64_t> fid;
static std::atomic<uint64_t> task_id;
bool SAVING_WHOLE; 
bool USE_CODECACHE;
bool sendZ3Solver(bool opti, SearchTask* task, 
          std::unordered_map<uint32_t, uint8_t> &solu, 
          uint64_t addr, bool solve);
void initZ3Solver();
void addCons(SearchTask* task);

struct RGDSolution {
  std::unordered_map<uint32_t, uint8_t> sol;
  //the intended branch for this solution
  uint32_t fid;  //the seed
  uint64_t addr;
  uint64_t ctx;
  uint32_t order;
  uint64_t direction;
  uint32_t field_index; //field start
  uint32_t field_size;  //field size
};

class SolutionQueue 
{
  std::deque<RGDSolution> queue_;
  std::mutex mutex_;
  std::condition_variable condvar_;

  typedef std::lock_guard<std::mutex> lock;
  typedef std::unique_lock<std::mutex> ulock;

  public:
  void push(RGDSolution const &val)
  {
    lock l(mutex_); // prevents multiple pushes corrupting queue_
    bool wake = queue_.empty(); // we may need to wake consumer
    queue_.push_back(val);
    if (wake) condvar_.notify_one();
  }


  RGDSolution pop()
  {
    lock l(mutex_);
    RGDSolution retval = queue_.front();
    queue_.pop_front();
    return retval;
  }

  void get_top_info(uint32_t* id, size_t* field_size, size_t* new_field_size)
  {
    ulock u(mutex_);
    while (queue_.empty())
      condvar_.wait(u);
    // now queue_ is non-empty and we still have the lock
    RGDSolution retval = queue_.front();
    *id = retval.fid;
    *field_size = retval.field_size;
    *new_field_size = retval.sol.size();
  }
};



SolutionQueue solution_queue;
TaskQueue task_queue;
void save_task(const unsigned char* input, unsigned int input_length) {
  CodedInputStream s(input,input_length);
  s.SetRecursionLimit(10000);
  SearchTask task;
  task.ParseFromCodedStream(&s);
  printTask(&task);
  saveRequest(task, "regression.data");
}

void handle_task_z3_sync(std::shared_ptr<SearchTask> task, bool solve) {
    std::unordered_map<uint32_t, uint8_t> z3_solution;

    //if (rgd_solutions.size() == 0) {
    bool ret = sendZ3Solver(false, task.get(), z3_solution, task->addr(), solve);
    if (!ret)
      sendZ3Solver(true, task.get(), z3_solution, task->addr(), solve);
    //}


    if (!SAVING_WHOLE) {
      if (z3_solution.size() != 0) {
        RGDSolution sol = {z3_solution, task->fid(), task->addr(), 
                        task->ctx(), task->order(), task->direction(),
                        0, z3_solution.size()};
        solution_queue.push(sol);
      }

    } else {
      std::string old_string = std::to_string(task->fid());
      std::string input_file = "corpus/angora/queue/id:" + 
                    std::string(6-old_string.size(),'0') + old_string;
      if (z3_solution.size() != 0)
        generate_input(z3_solution, input_file, "./raw_cases", fid++);

    }
  return;
}

void* handle_task_z3(void*) {
  //printTask(task.get());
  int solve_count = 0;
  uint64_t start = getTimeStamp();
  while (true) {
    auto task1 = task_queue.pop();
    
    std::shared_ptr<SearchTask> task = task1.first;
    bool solve = task1.second;


    std::unordered_map<uint32_t, uint8_t> z3_solution;

    //if (rgd_solutions.size() == 0) {
    bool ret = sendZ3Solver(false, task.get(), z3_solution, 
                            task->addr(), solve);
    if (!ret)
      sendZ3Solver(true, task.get(), z3_solution, task->addr(), solve);
    //}


    if (!SAVING_WHOLE) {
      if (z3_solution.size() != 0) {
        RGDSolution sol = {z3_solution, task->fid(), task->addr(), 
                          task->ctx(), task->order(), task->direction(),
                          0, z3_solution.size()};
        solution_queue.push(sol);
      }

    } else {
      std::string old_string = std::to_string(task->fid());
      std::string input_file = "corpus/angora/queue/id:" + 
                      std::string(6-old_string.size(),'0') + old_string;
      if (z3_solution.size() != 0)
        generate_input(z3_solution, input_file, "./raw_cases", fid++);

    }
    solve_count++; 
    if (solve_count % 10 == 0 && solve_count > 0) {
      uint64_t time_elapsed = getTimeStamp() - start;
      printf("solve count is %d flipping spped  %u/branch\n", 
              solve_count, time_elapsed/solve_count);
    }

  }
  return nullptr;
}

void handle_task_sync(std::shared_ptr<SearchTask> task, bool solve) {
    FUT* fut = nullptr;
    FUT* fut_opt = nullptr;
    if (!solve) {
	addCons(task.get());
  return;
    }

    lookup_or_construct(task.get(), &fut, &fut_opt, solve);

    std::vector<std::unordered_map<uint32_t, uint8_t>> rgd_solutions;
    std::vector<std::unordered_map<uint32_t, uint8_t>> partial_solutions;
    std::vector<std::unordered_map<uint32_t, uint8_t>> rgd_solutions_opt;
    fut->rgd_solutions = &rgd_solutions;
    fut->partial_solutions = &partial_solutions;
    fut_opt->rgd_solutions = &rgd_solutions_opt;
    if (solve) {
    gd_search(fut_opt);
    if (rgd_solutions_opt.size() != 0) {
      //fut->load_hint(rgd_solutions_opt[0]);
      gd_search(fut);
    }
     }

    if (!SAVING_WHOLE) {
      for (auto rgd_solution :  rgd_solutions) {
        RGDSolution sol = {rgd_solution, task->fid(), task->addr(), 
                        task->ctx(), task->order(), task->direction(),
                        0, rgd_solution.size()};
        solution_queue.push(sol);
      }

      for (auto rgd_solution :  rgd_solutions_opt) {
        RGDSolution sol = {rgd_solution, task->fid(), task->addr(), 
                        task->ctx(), task->order(), task->direction(),
                        0, rgd_solution.size()};
        solution_queue.push(sol);
      }
    } else {
      std::string old_string = std::to_string(task->fid());
      std::string input_file = "corpus/angora/queue/id:" + 
                    std::string(6-old_string.size(),'0') + old_string;
      for (auto rgd_solution : rgd_solutions) {
        generate_input(rgd_solution, input_file, "./raw_cases", fid++);
      }
      for (auto rgd_solution : rgd_solutions_opt) {
        generate_input(rgd_solution, input_file, "./raw_cases", fid++);
      }
    }

    delete fut;
    delete fut_opt;
}

//bool handle_task(int tid, std::shared_ptr<SearchTask> task) {
void* handle_task(void*) {
  int solve_count = 0;
  uint64_t start = getTimeStamp();
  while (true) {
    auto task1 = task_queue.pop();
    
    std::shared_ptr<SearchTask> task = task1.first;
    bool solve = task1.second;

    FUT* fut = nullptr;
    FUT* fut_opt = nullptr;
    if (!solve) {
	addCons(task.get());
	continue;
    }

    lookup_or_construct(task.get(), &fut, &fut_opt, task1.second);

    std::vector<std::unordered_map<uint32_t, uint8_t>> rgd_solutions;
    std::vector<std::unordered_map<uint32_t, uint8_t>> partial_solutions;
    std::vector<std::unordered_map<uint32_t, uint8_t>> rgd_solutions_opt;
    fut->rgd_solutions = &rgd_solutions;
    fut->partial_solutions = &partial_solutions;
    fut_opt->rgd_solutions = &rgd_solutions_opt;
    if (solve) {
    gd_search(fut_opt);
    if (rgd_solutions_opt.size() != 0) {
      //fut->load_hint(rgd_solutions_opt[0]);
      gd_search(fut);
    }
     }

    if (!SAVING_WHOLE) {
      for (auto rgd_solution :  rgd_solutions) {
        RGDSolution sol = {rgd_solution, task->fid(), task->addr(), 
                      task->ctx(), task->order(), task->direction(),
                        0, rgd_solution.size()};
        solution_queue.push(sol);
      }

      for (auto rgd_solution :  rgd_solutions_opt) {
        RGDSolution sol = {rgd_solution, task->fid(), task->addr(), 
                      task->ctx(), task->order(), task->direction(),
                        0, rgd_solution.size()};
        solution_queue.push(sol);
      }
    } else {
      std::string old_string = std::to_string(task->fid());
      std::string input_file = "corpus/angora/queue/id:" + 
                      std::string(6-old_string.size(),'0') + old_string;
      for (auto rgd_solution : rgd_solutions) {
        generate_input(rgd_solution, input_file, "./raw_cases", fid++);
      }
      for (auto rgd_solution : rgd_solutions_opt) {
        generate_input(rgd_solution, input_file, "./raw_cases", fid++);
      }
    }

    delete fut;
    delete fut_opt;
    solve_count++; 
    if (solve_count % 10 == 0 && solve_count > 0) {
      uint64_t time_elapsed = getTimeStamp() - start;
      printf("solve count is %d flipping speed  %u/branch\n", 
            solve_count, time_elapsed/solve_count);
    }
    
  }
  return nullptr;
}

  void init(bool saving_whole, bool use_codecache) {
    llvm::InitializeNativeTarget();
    llvm::InitializeNativeTargetAsmPrinter();
    llvm::InitializeNativeTargetAsmParser();
    JIT = std::move(GradJit::Create().get());
    pthread_t thread;
    pthread_attr_t attr;
    pthread_attr_init(&attr);
    pthread_attr_setdetachstate(&attr, PTHREAD_CREATE_JOINABLE);
    pthread_create(&thread, &attr, handle_task, nullptr);
    SAVING_WHOLE = saving_whole;
    USE_CODECACHE = use_codecache;
    initZ3Solver();
  }


  void handle_fmemcmp(uint8_t* data, uint32_t field_index, 
                    uint32_t field_size, uint32_t tid, uint64_t addr) {
    std::unordered_map<uint32_t, uint8_t> rgd_solution;
    std::string old_string = std::to_string(tid);
    std::string input_file = "corpus/angora/queue/id:" + 
                      std::string(6-old_string.size(),'0') + old_string;
    for(uint32_t i=0;i<strlen((const char *)data);i++) {
      rgd_solution[(uint32_t)field_index+i] = data[i];
    }
    //printf("index is %u and size is %u, data is %s, len is %d\n", 
     //           field_index,field_size,data, strlen((const char *)data));
    if (SAVING_WHOLE) {
      generate_input(rgd_solution, input_file, "./raw_cases", fid++);
    }
    else {
      RGDSolution sol = {rgd_solution, tid, addr, 0, 0, 0, field_index, field_size};
      solution_queue.push(sol);
    }
  }

  extern "C" {
    void submit_fmemcmp(uint8_t* data, uint32_t index, 
                  uint32_t size, uint32_t tid, uint64_t addr) {
      handle_fmemcmp(data,index,size, tid, addr);
    }

    void submit_task(const unsigned char* input, 
                    unsigned int input_length, bool solve) {
      CodedInputStream s(input,input_length);
      s.SetRecursionLimit(10000);
      std::shared_ptr<SearchTask> task = std::make_shared<SearchTask>();
      task->ParseFromCodedStream(&s);
      //printTask(task.get());
      //    handle_task(0,task);
      //incoming_tasks.enqueue({task, fresh});
      //std::pair<std::shared_ptr<SearchTask>,bool> tt{task,solve};
      //task_queue.push(tt);
      handle_task_sync(task,solve);
    }

    void init_core(bool saving_whole, bool use_codecache) { 
      init(saving_whole, use_codecache); 
    }

    void get_next_input_info(uint32_t* id, 
                            size_t *field_size,
                            size_t *new_field_size) {
      solution_queue.get_top_info(id, field_size, new_field_size); 
    } 

    //if we increase the size of the seed, 
    //the new_size is the new size of the input zero_extended
    //if we decrease the size of the seed, 
    //the new_size is the size of the original input
    void get_next_input(unsigned char* input, uint64_t *addr, uint64_t *ctx, 
        uint32_t *order, uint32_t *fid, uint64_t *direction, size_t new_size) {

      RGDSolution item = solution_queue.pop();
      //the field size is changed
      if (item.field_size != item.sol.size()) {
        unsigned char tmp[new_size];
        memcpy(tmp, input, new_size);
        //copy bytes after field
        for(int i = 0; 
          i < new_size - item.field_index-item.field_size &&
          i < new_size - item.field_index-item.sol.size(); i++) {
          tmp[item.field_index + item.sol.size() + i] = 
              input[item.field_index + item.field_size + i];
        }
        for(auto it = item.sol.begin(); it != item.sol.end(); ++it) {
          if (it->first < new_size)
            tmp[it->first] = it->second;
        }
        memcpy(input, tmp, new_size);
      } else {
        for(auto it = item.sol.begin(); it != item.sol.end(); ++it) {
          if (it->first < new_size)
            input[it->first] = it->second;
        }
      }
      *addr = item.addr;
      *ctx = item.ctx;
      *order = item.order;
      *fid = item.fid;
      *direction = item.direction;
    }
  };

