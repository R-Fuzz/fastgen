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
using namespace rgd;
using namespace google::protobuf::io;

#define THREAD_POOL_SIZE 4

//global variables
std::unique_ptr<GradJit> JIT;
uint64_t fid = 0;
bool init = false;
ctpl::thread_pool* pool;

void save_task(const unsigned char* input, unsigned int input_length) {
  CodedInputStream s(input,input_length);
  s.SetRecursionLimit(10000);
  SearchTask task;
  task.ParseFromCodedStream(&s);
  printTask(&task);
  saveRequest(task, "test.data");
}


void handle_task(const unsigned char* input, unsigned int input_length) {
  CodedInputStream s(input,input_length);
  s.SetRecursionLimit(10000);
  SearchTask task;
  //printTask(&task);

  task.ParseFromCodedStream(&s);
  FUT* fut = construct_task(&task);
  std::unordered_map<uint32_t, uint8_t> rgd_solution;
  fut->rgd_solution = &rgd_solution;
  gd_search(fut);
  std::string old_string = std::to_string(task.fid());
  std::string input_file = "/home/cju/quickgen/test/output/queue/id:" + std::string(6-old_string.size(),'0') + old_string;
  //std::string input_file = "/home/cju/quickgen/test/input/small_exec.elf";
  //std::cout << "input file is " << input_file << std::endl;
  generate_input(rgd_solution, input_file, "/home/cju/test", fid++);
}


void init_searcher() {
  llvm::InitializeNativeTarget();
  llvm::InitializeNativeTargetAsmPrinter();
  llvm::InitializeNativeTargetAsmParser();
  JIT = std::move(GradJit::Create().get());
  pool = new ctpl::thread_pool(THREAD_POOL_SIZE,0);
}


extern "C" {
  void submit_task(const unsigned char* input, unsigned int input_length) {
    //    save_task(input,input_length);

    if (!init) {
      init = true;
      init_searcher();
    }
    handle_task(input,input_length);

  }
};

