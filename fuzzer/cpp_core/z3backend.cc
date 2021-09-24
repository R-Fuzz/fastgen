#include <unordered_map>
#include <unordered_set>
#include <stdio.h>
#include "util.h"
#include "union_table.h"
#include "rgd_op.h"
#include <z3++.h>
#include <sys/types.h>
#include <sys/shm.h>
#include <iostream>
#include <fstream>
#include <fcntl.h>           /* For O_* constants */
#include <sys/stat.h>        /* For mode constants */
#include <semaphore.h>
#include <stdlib.h>
#include <math.h>
#include <string.h>
#include <stdio.h>
#include "util.h"
#include "interface.h"
#include <vector>
#include <string>
#include <unistd.h>
#include <deque>
#include <condition_variable>
#include <atomic>
#include <mutex>

#define THREAD_POOL_SIZE 1
#define DEBUG 1

#define B_FLIPPED 0x1
//global variables

static std::atomic<uint64_t> fid;
bool SAVING_WHOLE; 
z3::context *__z3_context;
z3::solver *__z3_solver; //(__z3_context, "QF_BV");
static const dfsan_label kInitializingLabel = -1;
static uint32_t max_label_per_session = 0;
sem_t * semagra;
sem_t * semace;
uint32_t total_generation_count = 0;
uint32_t normal_constraints = 0;
uint32_t size_constraints = 0;
uint32_t offset_constraints = 0;
uint64_t total_time = 0;

static dfsan_label_info *__union_table;

struct RGDSolution {
  std::unordered_map<uint32_t, uint8_t> sol;
  //the intended branch for this solution
  uint32_t fid;  //the seed
  uint64_t addr;
  uint64_t ctx;
  uint32_t order;
  uint64_t direction;
  uint32_t bid;
  uint32_t sctx;
  bool is_cmp;
  uint32_t predicate;
  uint64_t target_cond;
  uint32_t cons_hash;
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

  uint32_t get_top_id()
  {
    ulock u(mutex_);
    while (queue_.empty())
      condvar_.wait(u);
    // now queue_ is non-empty and we still have the lock
    RGDSolution retval = queue_.front();
    return retval.fid;
  }
};

SolutionQueue solution_queue;

// dependencies
struct dedup_hash {
  std::size_t operator()(const std::tuple<uint64_t,uint64_t,uint64_t,uint32_t> &operand) const {
    return std::hash<uint64_t>{}(std::get<0>(operand))^
      std::hash<uint64_t>{}(std::get<1>(operand))^
      std::hash<uint64_t>{}(std::get<2>(operand))^
      std::hash<uint32_t>{}(std::get<3>(operand));
  }
};

struct dedup_equal {
  bool operator()(const std::tuple<uint64_t,uint64_t,uint64_t,uint32_t> &lhs, const std::tuple<uint64_t,uint64_t,uint64_t,uint32_t> &rhs) const {
    return std::get<0>(lhs) == std::get<0>(rhs) && 
      std::get<1>(lhs) == std::get<1>(rhs) && 
      std::get<2>(lhs) == std::get<2>(rhs) && 
      std::get<3>(lhs) == std::get<3>(rhs);
  }
};

//static std::unordered_map<std::tuple<uint64_t, uint64_t, uint64_t, uint32_t>, uint32_t, dedup_hash, dedup_equal> global_counter;

// this maps key -> index in global_counter
static std::unordered_map<std::tuple<uint64_t, uint64_t, uint64_t, uint32_t>, uint32_t, dedup_hash, dedup_equal> global_hash_map;

static std::vector<uint32_t> global_counter;

//seed_id -> vector of indics
static std::unordered_map<uint32_t, std::vector<uint32_t>> seed_map;

//bcount filter  <pc,ctx,direction, local_bucket_count>
std::mutex bcount_mutex;
static std::unordered_map<std::tuple<uint64_t, uint64_t, uint64_t, uint32_t>, uint32_t, dedup_hash, dedup_equal> bcount_dedup;

std::unordered_map<uint32_t,z3::expr> expr_cache;
std::unordered_map<uint32_t,uint32_t> tsize_cache;
std::unordered_map<uint32_t,std::unordered_set<uint32_t>> deps_cache;


// dependencies
struct expr_hash {
  std::size_t operator()(const z3::expr &expr) const {
    return expr.hash();
  }
};
struct expr_equal {
  bool operator()(const z3::expr &lhs, const z3::expr &rhs) const {
    return lhs.id() == rhs.id();
  }
};
typedef std::unordered_set<z3::expr, expr_hash, expr_equal> expr_set_t;
struct branch_dep_t {
  expr_set_t expr_deps;
  std::unordered_set<dfsan_label> input_deps;
};
static std::unordered_map<size_t, branch_dep_t> branch_deps;

static inline dfsan_label_info* get_label_info(dfsan_label label) {
  return &__union_table[label];
}

static z3::expr get_cmd(z3::expr const &lhs, z3::expr const &rhs, uint32_t predicate) {
  switch (predicate) {
    case DFSAN_BVEQ:  return lhs == rhs;
    case DFSAN_BVNEQ: return lhs != rhs;
    case DFSAN_BVUGT: return z3::ugt(lhs, rhs);
    case DFSAN_BVUGE: return z3::uge(lhs, rhs);
    case DFSAN_BVULT: return z3::ult(lhs, rhs);
    case DFSAN_BVULE: return z3::ule(lhs, rhs);
    case DFSAN_BVSGT: return lhs > rhs;
    case DFSAN_BVSGE: return lhs >= rhs;
    case DFSAN_BVSLT: return lhs < rhs;
    case DFSAN_BVSLE: return lhs <= rhs;
    default:
                      printf("FATAL: unsupported predicate: %u\n", predicate);
                      throw z3::exception("unsupported predicate");
                      break;
  }
  // should never reach here
  throw z3::exception("unsupported predicate");
}

static inline z3::expr cache_expr(dfsan_label label, z3::expr const &e, std::unordered_set<uint32_t> &deps) {
  if (label != 0)  {
    expr_cache.insert({label,e});
    deps_cache.insert({label,deps});
  }
  return e;
}

static z3::expr serialize(dfsan_label label, std::unordered_set<uint32_t> &deps) {
  if (label < CONST_OFFSET || label == kInitializingLabel) {
    printf("WARNING: invalid label: %d\n", label);
    throw z3::exception("invalid label");
  }


  if (label > max_label_per_session)
    max_label_per_session = label;


  dfsan_label_info *info = get_label_info(label);

  if (info->depth > 500) {
    printf("WARNING: tree depth too large: %d\n", info->depth);
    throw z3::exception("tree too deep");
  }
  //printf("%u = (l1:%u, l2:%u, op:%u, size:%u, op1:%llu, op2:%llu)\n",
  //   label, info->l1, info->l2, info->op, info->size, info->op1, info->op2);

  auto itr_expr = expr_cache.find(label);
  auto itr_deps = deps_cache.find(label);
  if (label !=0 && itr_expr != expr_cache.end() && itr_deps != deps_cache.end() ) {
    //auto d = reinterpret_cast<std::unordered_set<uint32_t>*>(info->deps);
    deps.insert(itr_deps->second.begin(), itr_deps->second.end());
    //return *reinterpret_cast<z3::expr*>(info->expr);
    return itr_expr->second;
  }

  // special ops
  if (info->op == 0) {
    // input
    z3::symbol symbol = __z3_context->int_symbol(info->op1);
    z3::sort sort = __z3_context->bv_sort(8);
    //info->tree_size = 1; // lazy init
    deps.insert(info->op1);
    // caching is not super helpful
    return __z3_context->constant(symbol, sort);
  } else if (info->op == DFSAN_LOAD) {
    uint64_t offset = get_label_info(info->l1)->op1;
    z3::symbol symbol = __z3_context->int_symbol(offset);
    z3::sort sort = __z3_context->bv_sort(8);
    z3::expr out = __z3_context->constant(symbol, sort);
    deps.insert(offset);
    for (uint32_t i = 1; i < info->l2; i++) {
      symbol = __z3_context->int_symbol(offset + i);
      out = z3::concat(__z3_context->constant(symbol, sort), out);
      deps.insert(offset + i);
    }
    //info->tree_size = 1; // lazy init
    return cache_expr(label, out, deps);
  } else if (info->op == DFSAN_ZEXT) {
    z3::expr base = serialize(info->l1, deps);
    if (base.is_bool()) // dirty hack since llvm lacks bool
      base = z3::ite(base, __z3_context->bv_val(1, 1),
          __z3_context->bv_val(0, 1));
    uint32_t base_size = base.get_sort().bv_size();
    //info->tree_size = get_label_info(info->l1)->tree_size; // lazy init
    return cache_expr(label, z3::zext(base, info->size - base_size), deps);
  } else if (info->op == DFSAN_SEXT) {
    z3::expr base = serialize(info->l1, deps);
    if (base.is_bool()) // dirty hack since llvm lacks bool
      base = z3::ite(base, __z3_context->bv_val(1, 1),
          __z3_context->bv_val(0, 1));
    uint32_t base_size = base.get_sort().bv_size();
    //info->tree_size = get_label_info(info->l1)->tree_size; // lazy init
    return cache_expr(label, z3::sext(base, info->size - base_size), deps);
  } else if (info->op == DFSAN_TRUNC) {
    z3::expr base = serialize(info->l1, deps);
    //info->tree_size = get_label_info(info->l1)->tree_size; // lazy init
    return cache_expr(label, base.extract(info->size - 1, 0), deps);
  } else if (info->op == DFSAN_EXTRACT) {
    z3::expr base = serialize(info->l1, deps);
    //info->tree_size = get_label_info(info->l1)->tree_size; // lazy init
    return cache_expr(label, base.extract((info->op2 + info->size) - 1, info->op2), deps);
  } else if (info->op == DFSAN_NOT) {
    if (info->l2 == 0 || info->size != 1) {
      throw z3::exception("invalid Not operation");
    }
    z3::expr e = serialize(info->l2, deps);
    //info->tree_size = get_label_info(info->l2)->tree_size; // lazy init
    if (!e.is_bool()) {
      throw z3::exception("Only LNot should be recorded");
    }
    return cache_expr(label, !e, deps);
  } else if (info->op == DFSAN_NEG) {
    if (info->l2 == 0) {
      throw z3::exception("invalid Neg predicate");
    }
    z3::expr e = serialize(info->l2, deps);
    //info->tree_size = get_label_info(info->l2)->tree_size; // lazy init
    return cache_expr(label, -e, deps);
  }
  // common ops
  uint8_t size = info->size;
  // size for concat is a bit complicated ...
  if (info->op == DFSAN_CONCAT && info->l1 == 0) {
    assert(info->l2 >= CONST_OFFSET);
    size = info->size - get_label_info(info->l2)->size;
  }
  z3::expr op1 = __z3_context->bv_val((uint64_t)info->op1, size);
  if (info->l1 >= CONST_OFFSET) {
    op1 = serialize(info->l1, deps).simplify();
  } else if (info->size == 1) {
    op1 = __z3_context->bool_val(info->op1 == 1);
  }
  if (info->op == DFSAN_CONCAT && info->l2 == 0) {
    assert(info->l1 >= CONST_OFFSET);
    size = info->size - get_label_info(info->l1)->size;
  }
  z3::expr op2 = __z3_context->bv_val((uint64_t)info->op2, size);
  if (info->l2 >= CONST_OFFSET) {
    std::unordered_set<uint32_t> deps2;
    op2 = serialize(info->l2, deps2).simplify();
    deps.insert(deps2.begin(),deps2.end());
  } else if (info->size == 1) {
    op2 = __z3_context->bool_val(info->op2 == 1); }
  // update tree_size
  //info->tree_size = get_label_info(info->l1)->tree_size +
  // get_label_info(info->l2)->tree_size;

  switch((info->op & 0xff)) {
    // llvm doesn't distinguish between logical and bitwise and/or/xor
    case DFSAN_AND:     return cache_expr(label, info->size != 1 ? (op1 & op2) : (op1 && op2), deps);
    case DFSAN_OR:      return cache_expr(label, info->size != 1 ? (op1 | op2) : (op1 || op2), deps);
    case DFSAN_XOR:     return cache_expr(label, op1 ^ op2, deps);
    case DFSAN_SHL:     return cache_expr(label, z3::shl(op1, op2), deps);
    case DFSAN_LSHR:    return cache_expr(label, z3::lshr(op1, op2), deps);
    case DFSAN_ASHR:    return cache_expr(label, z3::ashr(op1, op2), deps);
    case DFSAN_ADD:     return cache_expr(label, op1 + op2, deps);
    case DFSAN_SUB:     return cache_expr(label, op1 - op2, deps);
    case DFSAN_MUL:     return cache_expr(label, op1 * op2, deps);
    case DFSAN_UDIV:    return cache_expr(label, z3::udiv(op1, op2), deps);
    case DFSAN_SDIV:    return cache_expr(label, op1 / op2, deps);
    case DFSAN_UREM:    return cache_expr(label, z3::urem(op1, op2), deps);
    case DFSAN_SREM:    return cache_expr(label, z3::srem(op1, op2), deps);
                        // relational
    case DFSAN_ICMP:    return cache_expr(label, get_cmd(op1, op2, info->op >> 8), deps);
                        // concat
    case DFSAN_CONCAT:  return cache_expr(label, z3::concat(op2, op1), deps); // little endian
    default:
                        printf("FATAL: unsupported op: %u\n", info->op);
                        throw z3::exception("unsupported operator");
                        break;
  }
 
  throw z3::exception("invalid label"); 
}

void init(bool saving_whole) {
  SAVING_WHOLE = saving_whole;
}

static void generate_solution(z3::model &m, std::unordered_map<uint32_t, uint8_t> &solu) {
  unsigned num_constants = m.num_consts();
  for(unsigned i = 0; i< num_constants; i++) {
    z3::func_decl decl = m.get_const_decl(i);
    z3::expr e = m.get_const_interp(decl);
    z3::symbol name = decl.name();
    if(name.kind() == Z3_INT_SYMBOL) {
      uint8_t value = (uint8_t)e.get_numeral_int();
      solu[name.to_int()] = value;
      //std::cout << " generate_input index is " << name.to_int() << " and value is " << (int)value << std::endl;
    }
  }
}

static void solve_gep(dfsan_label label, uint64_t r, bool try_solve, uint32_t tid,
    std::unordered_map<uint32_t, uint8_t> &sol,
    std::unordered_map<uint32_t, uint8_t> &opt_sol) {

  if (label == 0)
    return;

  if ((get_label_info(label)->flags & B_FLIPPED))
    return;


  unsigned char size = get_label_info(label)->size;

  try {
    std::unordered_set<dfsan_label> inputs;
    z3::expr index = serialize(label, inputs);
    z3::expr result = __z3_context->bv_val((uint64_t)r, size);
    if (try_solve) {
      // collect additional input deps
      std::vector<dfsan_label> worklist;
      worklist.insert(worklist.begin(), inputs.begin(), inputs.end());
      while (!worklist.empty()) {
        auto off = worklist.back();
        worklist.pop_back();

        auto &deps = branch_deps[off];
        for (auto i : deps.input_deps) {
          if (inputs.insert(i).second)
            worklist.push_back(i);
        }
      }

      __z3_solver->reset();

      __z3_solver->add(index > result);
      z3::check_result res = __z3_solver->check();

      //AOUT("\n%s\n", __z3_solver->to_smt2().c_str());
      if (res == z3::sat) {
        z3::model m_opt = __z3_solver->get_model();
        __z3_solver->push();

        // 2. add constraints
        expr_set_t added;
        for (auto off : inputs) {
          auto &deps = branch_deps[off];
          for (auto &expr : deps.expr_deps) {
            if (added.insert(expr).second) {
              __z3_solver->add(expr);
            }
          }
        }

        res = __z3_solver->check();
        if (res == z3::sat) {
          z3::model m = __z3_solver->get_model();
          sol.clear();
          generate_solution(m, sol);
        } else {
          opt_sol.clear();
          generate_solution(m_opt, opt_sol);
        }
      }
    }
    // preserve
    for (auto off : inputs) {
      auto &deps = branch_deps[off];
      deps.input_deps.insert(inputs.begin(), inputs.end());
      deps.expr_deps.insert(index == result);
    }

  } catch (z3::exception e) {
    printf("WARNING: index solving error: %s\n", e.msg());
    //printf("Expr is %s\n", __z3_solver->to_smt2().c_str());
  }

  return;
}


static void solve_cond(dfsan_label label, uint32_t direction,
    std::unordered_map<uint32_t, uint8_t> &opt_sol, 
    std::unordered_map<uint32_t, uint8_t> &sol, bool try_solve) {

  z3::expr result = __z3_context->bool_val(direction);

  if (!label) 
    return;

  try {
    std::unordered_set<dfsan_label> inputs;
    z3::expr cond = serialize(label, inputs);
    if(try_solve) {
#if 0
      if (get_label_info(label)->tree_size > 50000) {
        // don't bother?
        throw z3::exception("formula too large");
      }
#endif

      // collect additional input deps
      std::vector<dfsan_label> worklist;
      worklist.insert(worklist.begin(), inputs.begin(), inputs.end());
      while (!worklist.empty()) {
        auto off = worklist.back();
        worklist.pop_back();

        auto &deps = branch_deps[off];
        for (auto i : deps.input_deps) {
          if (inputs.insert(i).second)
            worklist.push_back(i);
        }
      }

      __z3_solver->reset();
      //AOUT("%s\n", cond.to_string().c_str());
      __z3_solver->add(cond != result);
      //z3::check_result res = __z3_solver->check();
      //if (res == z3::sat) {
      //  z3::model m_opt = __z3_solver->get_model();
      //  __z3_solver->push();

      // 2. add constraints
      expr_set_t added;
      for (auto off : inputs) {
        //AOUT("adding offset %d\n", off);
        auto &deps = branch_deps[off];
        for (auto &expr : deps.expr_deps) {
          if (added.insert(expr).second) {
            //AOUT("adding expr: %s\n", expr.to_string().c_str());
            __z3_solver->add(expr);
          }
        }
      } 
      z3::check_result res = __z3_solver->check();
      //printf("\n%s\n", __z3_solver->to_smt2().c_str()); 
      if (res == z3::sat) {
        z3::model m = __z3_solver->get_model();
        generate_solution(m, sol);
      } else {
        __z3_solver->reset();
        //AOUT("%s\n", cond.to_string().c_str());
        __z3_solver->add(cond != result);
        res = __z3_solver->check();

        if (res == z3::sat) {
          z3::model m_opt = __z3_solver->get_model();
          generate_solution(m_opt, opt_sol);
        }

      }
      //}
    } //end of try_solve
    //nested branches
    for (auto off : inputs) {
      auto &dep = branch_deps[off];
      dep.input_deps.insert(inputs.begin(), inputs.end());
      dep.expr_deps.insert(cond == result);
    }
  } catch (z3::exception e) {
    printf("WARNING: solving error: %s\n", e.msg());
    //printf("Expr is %s\n", __z3_solver->to_smt2().c_str());
  }
  return;
}


//check if we need to solve a branch given
// labe: if 0 concreate
// addr: branch address
// output: true: solve the constraints false: don't solve the constraints
static uint8_t COUNT_LOOKUP[256] = {
  0, 1, 2, 4, 8, 8, 8, 8, 16, 16, 16, 16, 16, 16, 16, 16, 32, 32, 32, 32, 32, 32, 32, 32, 32, 32,
  32, 32, 32, 32, 32, 32, 64, 64, 64, 64, 64, 64, 64, 64, 64, 64, 64, 64, 64, 64, 64, 64, 64, 64,
  64, 64, 64, 64, 64, 64, 64, 64, 64, 64, 64, 64, 64, 64, 64, 64, 64, 64, 64, 64, 64, 64, 64, 64,
  64, 64, 64, 64, 64, 64, 64, 64, 64, 64, 64, 64, 64, 64, 64, 64, 64, 64, 64, 64, 64, 64, 64, 64,
  64, 64, 64, 64, 64, 64, 64, 64, 64, 64, 64, 64, 64, 64, 64, 64, 64, 64, 64, 64, 64, 64, 64, 64,
  64, 64, 64, 64, 64, 64, 128, 128, 128, 128, 128, 128, 128, 128, 128, 128, 128, 128, 128, 128,
  128, 128, 128, 128, 128, 128, 128, 128, 128, 128, 128, 128, 128, 128, 128, 128, 128, 128, 128,
  128, 128, 128, 128, 128, 128, 128, 128, 128, 128, 128, 128, 128, 128, 128, 128, 128, 128, 128,
  128, 128, 128, 128, 128, 128, 128, 128, 128, 128, 128, 128, 128, 128, 128, 128, 128, 128, 128,
  128, 128, 128, 128, 128, 128, 128, 128, 128, 128, 128, 128, 128, 128, 128, 128, 128, 128, 128,
  128, 128, 128, 128, 128, 128, 128, 128, 128, 128, 128, 128, 128, 128, 128, 128, 128, 128, 128,
  128, 128, 128, 128, 128, 128, 128, 128, 128, 128, 128, 128, 128, 128, 128, 128, 128, 128, 128,
};


bool bcount_filter(uint64_t addr, uint64_t ctx, uint64_t direction, uint32_t order) {
  //uint32_t bucket = COUNT_LOOKUP[order-1]; 
  std::tuple<uint64_t,uint64_t, uint64_t, uint8_t> key{addr,ctx,direction,order};
  bcount_mutex.lock();
  bool res = false;
  auto itr = bcount_dedup.find(key);
  if (itr == bcount_dedup.end()) {
    bcount_dedup.insert({key, 1});
    res = true;
  } else {
    if (itr->second < 5) {
      res = true;
      itr->second++;  
    } else {
      res = false;
    }
  }
  bcount_mutex.unlock();
  return res;
}

void insert_flip_status(uint64_t addr, uint64_t ctx, uint64_t direction, uint32_t order) {
  //uint32_t bucket = COUNT_LOOKUP[order-1]; 
  std::tuple<uint64_t,uint64_t, uint64_t, uint32_t> key{addr,ctx,direction,order};
  bcount_mutex.lock();
  bcount_dedup[key] = 10;
  bcount_mutex.unlock();
  return;
}


void handle_fmemcmp(uint8_t* data, uint32_t label, 
    uint64_t size, uint32_t tid, uint64_t addr,
    std::unordered_map<uint32_t, uint8_t> &sol) {
  //concrete
  try {
    z3::expr op_concrete = __z3_context->bv_val(*data++, 8);
    for (uint8_t i = 1; i < size; i++) {
      op_concrete = z3::concat(__z3_context->bv_val(*data++, 8), op_concrete);
    }

    std::unordered_set<dfsan_label> inputs;
    z3::expr op_symbolic = serialize(label, inputs);
    __z3_solver->reset();
    __z3_solver->add(op_symbolic == op_concrete);
    z3::check_result res = __z3_solver->check();
    if (res == z3::sat) {
      z3::model m = __z3_solver->get_model();
      generate_solution(m, sol);
    }
  } catch (z3::exception e) {
    printf("WARNING: solving error: %s\n", e.msg());
  }
}


void cleanup() {
  expr_cache.clear();
  deps_cache.clear();
  max_label_per_session = 0;
  branch_deps.clear();
  shmdt(__union_table);
  delete __z3_solver;
  delete __z3_context;
  return;
}

void solve(int shmid, int pipefd) {
  // map the union table
  std::cerr << "shmid " << shmid << " pipefd " <<pipefd << std::endl;
  __union_table = (dfsan_label_info*)shmat(shmid, nullptr, SHM_RDONLY);
  if (__union_table == (void*)(-1)) {
    printf("error %s\n",strerror(errno));
    return;
  }
  __z3_context = new z3::context();
  __z3_solver = new z3::solver(*__z3_context, "QF_BV");
  __z3_solver->set("timeout", 10000U);
  size_t pos = 0;

  uint32_t count = 0;
  //create global state for one session
  //XXH64_state_t path_prefix;
  //XXH64_reset(&path_prefix,0);
  uint64_t acc_time = 0;
  uint64_t one_start = getTimeStamp();
  bool skip_rest = false;
  std::vector<uint32_t> per_seed_keys;
  struct pipe_msg msg;
  while (read(pipefd,&msg,sizeof(msg)) == sizeof(msg))
  {
    /*
       printf("read %d bytes\n", sizeof(msg)); 
       std::cout << "tid: " << msg.tid
       << " label: " << msg.label
       << " result: " << msg.result
       << " addr: " << msg.addr
       << " ctx: " << msg.ctx
       << " localcnt: " << msg.localcnt
       << " type: " << msg.type << std::endl;
     */



    //the last token
    //std::cout << line << std::endl;
    //path prefix filter based on address of the branch
    //bool try_solve = filter(addr, label, direction, &path_prefix);
    std::unordered_map<uint32_t, uint8_t> sol;
    std::unordered_map<uint32_t, uint8_t> opt_sol;
    uint32_t cons_hash = 0;
    if (msg.type == 0) {  //cond
      if (skip_rest) continue;
      bool try_solve = bcount_filter(msg.addr, msg.ctx, 0, msg.localcnt);
      uint64_t tstart = getTimeStamp();
      solve_cond(msg.label, msg.result, opt_sol, sol, try_solve);
      dfsan_label_info *info = get_label_info(msg.label);
      if (info) {
        cons_hash = info->hash;
      }
      acc_time += getTimeStamp() - tstart;
      normal_constraints += 1;
      if (acc_time > 180000000 || count > 5000 ) //90s
        skip_rest = true;
    } else if (msg.type == 1) { //gep constraint
      bool try_solve = bcount_filter(msg.addr, msg.ctx, 0, msg.localcnt);
      offset_constraints += 1;
      solve_gep(msg.label, msg.result, try_solve, msg.tid, sol, opt_sol);
    } else if (msg.type == 2) {  //strcmp
      uint8_t data[msg.result];
      if (read(pipefd, data, msg.result) == msg.result) {
        size_constraints += 1;    
        bool try_solve = bcount_filter(msg.addr, msg.ctx, 0, msg.localcnt);
        if (try_solve)
          handle_fmemcmp(data, msg.label, msg.result, msg.tid, msg.addr, sol);
      } else {
        // pipe corruption
        break;
      }
    } else if (msg.type == 3) {
      offset_constraints += 1;
    } else if (msg.type == 4) {
      size_constraints += 1;
    }


    if (sol.size()) {
      RGDSolution rsol = {sol, msg.tid, msg.addr, msg.ctx, msg.localcnt, msg.result, 
        msg.bid, msg.sctx, msg.type == 0, msg.predicate, msg.target_cond, cons_hash};
      solution_queue.push(rsol);
      count++;
    }

    if (opt_sol.size()) {
      RGDSolution rsol = {opt_sol, msg.tid, msg.addr, msg.ctx, msg.localcnt, msg.result, 
        msg.bid, msg.sctx, msg.type == 0, msg.predicate, msg.target_cond, cons_hash};
      solution_queue.push(rsol);
      count++;
    }

  }
  total_generation_count += count;
  total_time += getTimeStamp() - one_start;
  cleanup();
  if (skip_rest) std::cout << "timeout!" << std::endl;
  std::cerr << "generate count " << count 
    << " total count " << total_generation_count 
    << " process_time " << getTimeStamp() - one_start 
    << " normal: " << normal_constraints
    << " size: " << size_constraints
    << " offset: " << offset_constraints
    << std::endl;
  return;
}



extern "C" {
  void init_core(bool saving_whole) { 
    init(saving_whole); 
    printf("the length of union_table is %u\n", 0xC00000000/sizeof(dfsan_label_info));
    return;
  }

  void run_solver(int shmid, uint32_t pipefd) {
    solve(shmid, pipefd);
    return;
  }

  void insert_flip(uint64_t addr, uint64_t ctx, uint64_t direction, uint32_t order) {
    insert_flip_status(addr,ctx,direction,order);
    return;
  }

  void get_next_input(unsigned char* input, uint64_t *addr, uint64_t *ctx, 
      uint32_t *order, uint32_t *fid, uint64_t *direction, 
      uint32_t* bid, uint32_t* sctx, bool* is_cmp, uint32_t* predicate, uint64_t* target_cond, uint32_t* cons_hash, size_t size) {
    //std::pair<uint32_t, std::unordered_map<uint32_t, uint8_t>> item;
    // printf("get_next_loop and queue size is %u\n", solution_queue.size_approx());
    //asert(!solutio_queue.empty());
    RGDSolution item = solution_queue.pop(); 
    for(auto it = item.sol.begin(); it != item.sol.end(); ++it) {
      if (it->first < size)
        input[it->first] = it->second;
    }
    *addr = item.addr;
    *ctx = item.ctx;
    *order = item.order;
    *fid = item.fid;
    *direction = item.direction;
    *bid = item.bid;
    *sctx = item.sctx;
    *is_cmp = item.is_cmp;
    *predicate = item.predicate;
    *target_cond = item.target_cond;
    *cons_hash = item.cons_hash;
    return;
  }

  uint32_t get_next_input_id() {
    return solution_queue.get_top_id();
  }

};

