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
#include <atomic>
#include <mutex>

#define THREAD_POOL_SIZE 1
#define DEBUG 1

#define B_FLIPPED 0x1
//global variables

static dfsan_label divisor_label = 0;
static std::atomic<uint64_t> fid;
bool SAVING_WHOLE; 
static z3::context __z3_context;
static z3::solver __z3_solver(__z3_context, "QF_BV");
static const dfsan_label kInitializingLabel = -1;
static uint32_t max_label_per_session = 0;
sem_t * semagra;
sem_t * semace;
uint32_t total_generation_count = 0;
uint64_t total_time = 0;

std::string input_file = "./corpus/angora/tmp/cur_input_2";
//std::string input_file = "/magma_shared/findings/tmp/cur_input_2";
static dfsan_label_info *__union_table;

struct RGDSolution {
  std::unordered_map<uint32_t, uint8_t> sol;
  //the intended branch for this solution
  uint32_t fid;  //the seed
  uint64_t addr;
  uint64_t ctx;
  uint32_t order;
  uint64_t direction;
};

std::deque<RGDSolution> solution_queue;
std::mutex queue_mutex;


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
static std::unordered_set<std::tuple<uint64_t, uint64_t, uint64_t, uint32_t>, dedup_hash, dedup_equal> fmemcmp_dedup;

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
  //Die();
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
    z3::symbol symbol = __z3_context.int_symbol(info->op1);
    z3::sort sort = __z3_context.bv_sort(8);
    //info->tree_size = 1; // lazy init
    deps.insert(info->op1);
    // caching is not super helpful
    return __z3_context.constant(symbol, sort);
  } else if (info->op == DFSAN_LOAD) {
    uint64_t offset = get_label_info(info->l1)->op1;
    z3::symbol symbol = __z3_context.int_symbol(offset);
    z3::sort sort = __z3_context.bv_sort(8);
    z3::expr out = __z3_context.constant(symbol, sort);
    deps.insert(offset);
    for (uint32_t i = 1; i < info->l2; i++) {
      symbol = __z3_context.int_symbol(offset + i);
      out = z3::concat(__z3_context.constant(symbol, sort), out);
      deps.insert(offset + i);
    }
    //info->tree_size = 1; // lazy init
    return cache_expr(label, out, deps);
  } else if (info->op == DFSAN_ZEXT) {
    z3::expr base = serialize(info->l1, deps);
    if (base.is_bool()) // dirty hack since llvm lacks bool
      base = z3::ite(base, __z3_context.bv_val(1, 1),
          __z3_context.bv_val(0, 1));
    uint32_t base_size = base.get_sort().bv_size();
    //info->tree_size = get_label_info(info->l1)->tree_size; // lazy init
    return cache_expr(label, z3::zext(base, info->size - base_size), deps);
  } else if (info->op == DFSAN_SEXT) {
    z3::expr base = serialize(info->l1, deps);
    if (base.is_bool()) // dirty hack since llvm lacks bool
      base = z3::ite(base, __z3_context.bv_val(1, 1),
          __z3_context.bv_val(0, 1));
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
  z3::expr op1 = __z3_context.bv_val((uint64_t)info->op1, size);
  if (info->l1 >= CONST_OFFSET) {
    op1 = serialize(info->l1, deps).simplify();
  } else if (info->size == 1) {
    op1 = __z3_context.bool_val(info->op1 == 1);
  }
  if (info->op == DFSAN_CONCAT && info->l2 == 0) {
    assert(info->l1 >= CONST_OFFSET);
    size = info->size - get_label_info(info->l1)->size;
  }
  z3::expr op2 = __z3_context.bv_val((uint64_t)info->op2, size);
  if (info->l2 >= CONST_OFFSET) {
    std::unordered_set<uint32_t> deps2;
    op2 = serialize(info->l2, deps2).simplify();
    deps.insert(deps2.begin(),deps2.end());
  } else if (info->size == 1) {
    op2 = __z3_context.bool_val(info->op2 == 1); }
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
    case DFSAN_UDIV:    divisor_label = info->l2; return cache_expr(label, z3::udiv(op1, op2), deps);
    case DFSAN_SDIV:    divisor_label = info->l2; return cache_expr(label, op1 / op2, deps);
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
  // should never reach here
  //Die();
}

void init(bool saving_whole) {
  SAVING_WHOLE = saving_whole;
  //initZ3Solver();
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

static void solve_divisor() {
  if (divisor_label <= CONST_OFFSET) return;
  dfsan_label label = divisor_label;
  if ((get_label_info(label)->flags & B_FLIPPED))
    return;
  try {
    std::unordered_set<dfsan_label> inputs;
    z3::expr cond = serialize(label, inputs);
    std::unordered_map<uint32_t, uint8_t> opt_sol; 
    std::unordered_map<uint32_t, uint8_t> sol;
    //std::string input_file = "./corpus/tmp/cur_input_2";
    //std::string input_file = "/magma_shared/findings/tmp/cur_input_2";
    unsigned char size = get_label_info(label)->size;
#if 0
    if (get_label_info(label)->tree_size > 50000) {
      // don't bother?
      throw z3::exception("formula too large");
    }
#endif
    z3::expr zero_v = __z3_context.bv_val((uint64_t)0, size);

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

    __z3_solver.reset();
    //AOUT("%s\n", cond.to_string().c_str());
    __z3_solver.add(cond == zero_v);
    z3::check_result res = __z3_solver.check();
    if (res == z3::sat) {
      z3::model m_opt = __z3_solver.get_model();
      __z3_solver.push();

      // 2. add constraints
      expr_set_t added;
      for (auto off : inputs) {
        //AOUT("adding offset %d\n", off);
        auto &deps = branch_deps[off];
        for (auto &expr : deps.expr_deps) {
          if (added.insert(expr).second) {
            //AOUT("adding expr: %s\n", expr.to_string().c_str());
            __z3_solver.add(expr);
          }
        }
      } 
      res = __z3_solver.check();
      //printf("\n%s\n", __z3_solver.to_smt2().c_str()); 
      if (res == z3::sat) {
        z3::model m = __z3_solver.get_model();
        sol.clear();
        generate_solution(m, sol);
        //generate_input(sol, input_file, "./ce_output", fid++);
        RGDSolution rsol = {sol, 0, 0, 0, 0, 0};
        queue_mutex.lock();
        solution_queue.push_back(rsol);
        queue_mutex.unlock();
      } else {
        opt_sol.clear();
        generate_solution(m_opt, opt_sol);
        RGDSolution rsol = {opt_sol, 0, 0, 0, 0, 0};
        queue_mutex.lock();
        solution_queue.push_back(rsol);
        queue_mutex.unlock();
        //generate_input(opt_sol, input_file, "./ce_output", fid++);
      }
    }
  } catch (z3::exception e) {
    printf("WARNING: solving error: %s\n", e.msg());
    //printf("Expr is %s\n", __z3_solver.to_smt2().c_str());
  }

}

static void solve_gep(dfsan_label label, uint64_t r, bool try_solve, uint32_t tid) {

  if (label == 0 || !try_solve)
    return;
  //std::string input_file = "/magma_shared/findings/tmp/cur_input_2";
  //std::string input_file = "./corpus/tmp/cur_input_2";

  if ((get_label_info(label)->flags & B_FLIPPED))
    return;

  std::unordered_map<uint32_t, uint8_t> opt_sol; 
  std::unordered_map<uint32_t, uint8_t> sol;

  unsigned char size = get_label_info(label)->size;

  try {
    std::unordered_set<dfsan_label> inputs;
    z3::expr index = serialize(label, inputs);
    z3::expr result = __z3_context.bv_val((uint64_t)r, size);

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

    __z3_solver.reset();

    __z3_solver.add(index > result);
    z3::check_result res = __z3_solver.check();

    //AOUT("\n%s\n", __z3_solver.to_smt2().c_str());
    if (res == z3::sat) {
      z3::model m_opt = __z3_solver.get_model();
      __z3_solver.push();

      // 2. add constraints
      expr_set_t added;
      for (auto off : inputs) {
        auto &deps = branch_deps[off];
        for (auto &expr : deps.expr_deps) {
          if (added.insert(expr).second) {
            __z3_solver.add(expr);
          }
        }
      }

      res = __z3_solver.check();
      if (res == z3::sat) {
        z3::model m = __z3_solver.get_model();
        sol.clear();
        generate_solution(m, sol);
        RGDSolution rsol = {sol, tid, 0, 0, 0, 0};
        queue_mutex.lock();
        solution_queue.push_back(rsol);
        queue_mutex.unlock();
      } else {
        opt_sol.clear();
        generate_solution(m_opt, opt_sol);
        RGDSolution rsol = {opt_sol, tid, 0, 0, 0, 0};
        queue_mutex.lock();
        solution_queue.push_back(rsol);
        queue_mutex.unlock();
      }
    }

    {
      __z3_solver.reset();
      z3::expr zero_v = __z3_context.bv_val((uint64_t)0, size);
      __z3_solver.add(index < zero_v);
      z3::check_result res = __z3_solver.check();

      //AOUT("\n%s\n", __z3_solver.to_smt2().c_str());
      if (res == z3::sat) {
        z3::model m_opt = __z3_solver.get_model();
        __z3_solver.push();

        // 2. add constraints
        expr_set_t added;
        for (auto off : inputs) {
          auto &deps = branch_deps[off];
          for (auto &expr : deps.expr_deps) {
            if (added.insert(expr).second) {
              __z3_solver.add(expr);
            }
          }
        }

        res = __z3_solver.check();
        if (res == z3::sat) {
          z3::model m = __z3_solver.get_model();
          sol.clear();
          generate_solution(m, sol);
          RGDSolution rsol = {sol, tid, 0, 0, 0, 0};
          queue_mutex.lock();
          solution_queue.push_back(rsol);
          queue_mutex.unlock();
        } else {
          opt_sol.clear();
          generate_solution(m_opt, opt_sol);
          RGDSolution rsol = {opt_sol, tid, 0, 0, 0, 0};
          queue_mutex.lock();
          solution_queue.push_back(rsol);
          queue_mutex.unlock();
        }
      }
    }
    for (int i=0; i<128;i++)
    {
      __z3_solver.reset();
      z3::expr cur_v = __z3_context.bv_val((uint64_t)i, size);
      __z3_solver.add(index == cur_v);
      z3::check_result res = __z3_solver.check();

      //AOUT("\n%s\n", __z3_solver.to_smt2().c_str());
      if (res == z3::sat) {
        z3::model m_opt = __z3_solver.get_model();
        __z3_solver.push();

        // 2. add constraints
        expr_set_t added;
        for (auto off : inputs) {
          auto &deps = branch_deps[off];
          for (auto &expr : deps.expr_deps) {
            if (added.insert(expr).second) {
              __z3_solver.add(expr);
            }
          }
        }

        res = __z3_solver.check();
        if (res == z3::sat) {
          z3::model m = __z3_solver.get_model();
          sol.clear();
          generate_solution(m, sol);
          RGDSolution rsol = {sol, tid, 0, 0, 0, 0};
          queue_mutex.lock();
          solution_queue.push_back(rsol);
          queue_mutex.unlock();
        } else {
          opt_sol.clear();
          generate_solution(m_opt, opt_sol);
          RGDSolution rsol = {opt_sol, tid, 0, 0, 0, 0};
          queue_mutex.lock();
          solution_queue.push_back(rsol);
          queue_mutex.unlock();
        }
      }
    }


    // preserve
    for (auto off : inputs) {
      auto &deps = branch_deps[off];
      deps.input_deps.insert(inputs.begin(), inputs.end());
      deps.expr_deps.insert(index == result);
    }

    // mark as visited
    get_label_info(label)->flags |= B_FLIPPED;
  } catch (z3::exception e) {
    printf("WARNING: index solving error: %s\n", e.msg());
    //printf("Expr is %s\n", __z3_solver.to_smt2().c_str());
  }

}


static void solve_cond(dfsan_label label, uint32_t direction,
    std::unordered_map<uint32_t, uint8_t> &opt_sol, 
    std::unordered_map<uint32_t, uint8_t> &sol, bool try_solve) {

  z3::expr result = __z3_context.bool_val(direction);

  if (!label || !try_solve) 
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

      __z3_solver.reset();
      //AOUT("%s\n", cond.to_string().c_str());
      __z3_solver.add(cond != result);
      z3::check_result res = __z3_solver.check();
      if (res == z3::sat) {
        z3::model m_opt = __z3_solver.get_model();
        __z3_solver.push();

        // 2. add constraints
        expr_set_t added;
        for (auto off : inputs) {
          //AOUT("adding offset %d\n", off);
          auto &deps = branch_deps[off];
          for (auto &expr : deps.expr_deps) {
            if (added.insert(expr).second) {
              //AOUT("adding expr: %s\n", expr.to_string().c_str());
              __z3_solver.add(expr);
            }
          }
        } 
        res = __z3_solver.check();
        //printf("\n%s\n", __z3_solver.to_smt2().c_str()); 
        if (res == z3::sat) {
          z3::model m = __z3_solver.get_model();
          generate_solution(m, sol);
        } else {
          generate_solution(m_opt, opt_sol);
        }
      }
    } //end of try_solve
    //nested branches
    for (auto off : inputs) {
      auto &dep = branch_deps[off];
      dep.input_deps.insert(inputs.begin(), inputs.end());
      dep.expr_deps.insert(cond == result);
    }
  } catch (z3::exception e) {
    printf("WARNING: solving error: %s\n", e.msg());
    //printf("Expr is %s\n", __z3_solver.to_smt2().c_str());
  }
}

std::string get_current_dir() {
  char buff[FILENAME_MAX]; //create string buffer to hold path
  getcwd( buff, FILENAME_MAX );
  std::string current_working_dir(buff);
  return current_working_dir;
}

const int kMapSize  = 1<<27;

uint8_t pp_map[kMapSize];

bool check_pp(uint64_t digest) {
  uint32_t hash = digest % (kMapSize * CHAR_BIT);
  uint32_t idx = hash / CHAR_BIT;
  uint32_t mask = 1 << (hash % CHAR_BIT);
  return (pp_map[idx] & mask) == 0;
}

void mark_pp(uint64_t digest) {
  uint32_t hash = digest % (kMapSize * CHAR_BIT);
  uint32_t idx = hash / CHAR_BIT;
  uint32_t mask = 1 << (hash % CHAR_BIT);
  pp_map[idx] |= mask;
}

//check if we need to solve a branch given
// labe: if 0 concreate
// addr: branch address
// output: true: solve the constraints false: don't solve the constraints

bool bcount_filter(uint64_t addr, uint64_t ctx, uint64_t direction, uint32_t order) {
  std::tuple<uint64_t,uint64_t, uint64_t, uint32_t> key{addr,ctx,direction,order};
  if (fmemcmp_dedup.find(key) != fmemcmp_dedup.end()) {
    return false;
  } else {
    fmemcmp_dedup.insert(key);
    return true;
  }
}

#if 0
bool filter(uint32_t label, uint64_t addr, uint64_t direction, 
    XXH64_state_t* path_prefix) {

  //address
  XXH64_state_t tmp;
  XXH64_reset(&tmp, 0);

  XXH64_update(path_prefix, &addr, sizeof(addr));

  //create a tmp prefix

  uint8_t deter = 0;
  if (label == 0)  {
    deter = 1;
    XXH64_update(path_prefix, &deter, sizeof(deter));
    XXH64_update(path_prefix, &direction, sizeof(direction));
    return false;
  }

  uint64_t direction_sym = 1 - direction;

  //digest
  uint64_t taken_digest;
  uint64_t untaken_digest;

  XXH64_update(path_prefix, &deter, sizeof(deter));
  XXH64_copyState(&tmp,path_prefix);

  XXH64_update(&tmp, &direction_sym, sizeof(direction_sym));
  untaken_digest = XXH64_digest(&tmp); 

  XXH64_update(path_prefix, &direction, sizeof(direction));
  taken_digest = XXH64_digest(path_prefix); 

  mark_pp(taken_digest);
  return check_pp(untaken_digest);
}
#endif

//roll in branch
#if 0
uint64_t roll_in_pp(uint32_t label, uint64_t addr, uint64_t direction, 
    XXH64_state_t* path_prefix) {

  //address
  XXH64_state_t tmp;
  XXH64_reset(&tmp, 0);

  XXH64_update(path_prefix, &addr, sizeof(addr));

  //create a tmp prefix

  uint8_t deter = 0;
  if (label == 0)  {
    deter = 1;
    XXH64_update(path_prefix, &deter, sizeof(deter));
    XXH64_update(path_prefix, &direction, sizeof(direction));
    return 0;
  }

  uint64_t direction_sym = 1 - direction;

  //digest
  uint64_t taken_digest;
  uint64_t untaken_digest;

  XXH64_update(path_prefix, &deter, sizeof(deter));
  XXH64_copyState(&tmp,path_prefix);

  XXH64_update(&tmp, &direction_sym, sizeof(direction_sym));
  untaken_digest = XXH64_digest(&tmp); 

  XXH64_update(path_prefix, &direction, sizeof(direction));
  taken_digest = XXH64_digest(path_prefix); 

  mark_pp(taken_digest);
  return untaken_digest;

}
#endif


bool dry_run(int32_t round, uint32_t seed_id) {
  auto keys = seed_map.find(seed_id);
  if (keys == seed_map.end()) {
    return true;
  }
  for (auto key : keys->second) {
    //unlikely
    assert(key < global_counter.size());
    if (key >= global_counter.size()) {
      return true;
    }
    int32_t g_bucket = (uint32_t)log2(global_counter[key]);
    if (g_bucket <= round) {
      // as long as there's one branch can be sent to solver run it
      return true;
    }
  }
  return false; 
}


//the local_counter starting from 1
#if 0
bool hybrid_filter(uint64_t addr, uint64_t ctx, uint64_t direction,
    uint32_t local_count, int32_t round,
    uint32_t label,
    XXH64_state_t* path_prefix, std::vector<uint32_t> *per_seed_keys) {

  //first, I calculate the path prefix for the taken and untaken branch
  //and in this function, I mark the taken branch
  // and return the digest for the untkane branch
  if (!label) return false;
  uint64_t untaken_digest = roll_in_pp(addr,label,direction,path_prefix);

  if (!check_pp(untaken_digest)) return false;

  //assert(direction == 0 || direction == 1);

  int32_t bucket = (uint32_t)log2(local_count);
  //increase the global branch counter for the taken branch
  std::tuple<uint64_t,uint64_t, uint64_t, uint32_t> key{addr,ctx,direction,bucket};
  auto itr = global_hash_map.find(key);
  if (itr == global_hash_map.end())  {
    uint32_t index = global_counter.size();
    global_counter.push_back(1);
    global_hash_map.insert({key,index});
  } else  {
    global_counter[itr->second]++;
  }


  //I retrieve the global counter of the untaken branch
  std::tuple<uint64_t,uint64_t, uint64_t, uint32_t> invkey{addr,ctx,1- direction,bucket};
  auto itr_untaken = global_hash_map.find(invkey);
  if (itr_untaken == global_hash_map.end())  {
    uint32_t index = global_counter.size();
    global_counter.push_back(1);
    itr_untaken = global_hash_map.insert({invkey, index}).first;
  }

  //remember per seed's branches
  per_seed_keys->push_back(itr_untaken->second);
  //I calculate the bucket for the global untaken branch
  assert(itr_untaken->second < global_counter.size());
  int32_t g_bucket = (uint32_t)log2(global_counter[itr_untaken->second]);


  //we check if we are solving it in this round
  //assert(g_bucket >= round);
  if (g_bucket <= round) {
    //we are going to solve in this round, we then mark the pp
    mark_pp(untaken_digest);  
    //increase the untaken branch global counter
    global_counter[itr_untaken->second]++;
    return true;
  }
  return false;
}
#endif

void handle_fmemcmp(uint8_t* data, uint64_t index, uint32_t size, uint32_t tid, uint64_t addr) {
  std::unordered_map<uint32_t, uint8_t> rgd_solution;
  //std::string input_file = "/magma_shared/findings/tmp/cur_input_2";
  // std::string old_string = std::to_string(tid);
  //std::string input_file = "/home/cju/fastgen/tests/switch/input_switch/i";
  //std::string input_file = "corpus/angora/queue/id:" + std::string(6-old_string.size(),'0') + old_string;
  for(uint32_t i=0;i<size;i++) {
    //rgd_solution[(uint32_t)index+i] = (uint8_t) (data & 0xff);
    rgd_solution[(uint32_t)index+i] = data[i];
    //data = data >> 8 ;
  }
  RGDSolution sol = {rgd_solution, tid, addr, 0, 0, 0};
  queue_mutex.lock();
  solution_queue.push_back(sol);
  queue_mutex.unlock();
}


void cleanup() {
  expr_cache.clear();
  deps_cache.clear();
  max_label_per_session = 0;
  branch_deps.clear();
  shmdt(__union_table);
}

uint32_t solve(int shmid, int pipefd) {
  // map the union table
  __union_table = (dfsan_label_info*)shmat(shmid, nullptr, SHM_RDONLY);
  if (__union_table == (void*)(-1)) {
    printf("error %s\n",strerror(errno));
    return 0;
  }
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

    printf("read %d bytes\n", sizeof(msg)); 
    std::cout << "tid: " << msg.tid
      << " label: " << msg.label
      << " result: " << msg.result
      << " addr: " << msg.addr
      << " ctx: " << msg.ctx
      << " localcnt: " << msg.localcnt
      << " type: " << msg.type << std::endl;



    //the last token
    //std::cout << line << std::endl;
    //path prefix filter based on address of the branch
    //bool try_solve = filter(addr, label, direction, &path_prefix);
    std::unordered_map<uint32_t, uint8_t> sol;
    std::unordered_map<uint32_t, uint8_t> opt_sol;
    if (msg.type == 0) {  //cond
      if (skip_rest) continue;
      //bool try_solve = hybrid_filter(msg.addr, msg.ctx, msg.result, msg.localcnt, 
      //  label, &path_prefix, &per_seed_keys);
      //if (try_solve) filtered_count++;
      uint64_t tstart = getTimeStamp();
      solve_cond(msg.label, msg.result, opt_sol, sol, true);
      acc_time += getTimeStamp() - tstart;
      if (acc_time > 90000000 || count > 5000 ) //90s
        skip_rest = true;
      //if (try_solve)
      // solve_divisor();
    }
    else if (msg.type == 2) {  //strcmp
      uint8_t data[msg.result];
      if (read(pipefd, data, msg.result) == msg.result) {
        //bool try_solve = filter(addr, label, direction, &path_prefix);
        printf("read strcmp %d\n", msg.result);
        bool try_solve = bcount_filter(msg.addr, msg.ctx, 0, msg.localcnt);
        if (try_solve)
          handle_fmemcmp(data, msg.label, msg.result, msg.tid, msg.addr);
      } else {
        // pipe corruption
        break;
      }
    } else if (msg.type == 1) { //gep constraint
      bool try_solve = bcount_filter(msg.addr, msg.ctx, 0, msg.localcnt);
      if (try_solve)
        solve_gep(msg.label, msg.result, try_solve, msg.tid); 
    }

    if (sol.size()) {
      RGDSolution rsol = {sol, msg.tid, 0, 0, 0, 0};
      queue_mutex.lock();
      solution_queue.push_back(rsol);
      queue_mutex.unlock();
      count++;
    }
    if (opt_sol.size()) {
      RGDSolution rsol = {opt_sol, msg.tid, 0, 0, 0, 0};
      queue_mutex.lock();
      solution_queue.push_back(rsol);
      queue_mutex.unlock();
      count++;
    }
    /* 
       if (sol.size()) {
       generate_input(sol, input_file, "./ce_output", fid++);
       count++;
       }
       if (opt_sol.size()) {
       generate_input(opt_sol, input_file, "./ce_output", fid++);
       count++;
       }
     */
  }
  total_generation_count += count;
  total_time += getTimeStamp() - one_start;
  cleanup();
  if (skip_rest) std::cout << "timeout!" << std::endl;
  std::cout << "generate count " << count 
    << " total count " << total_generation_count 
    << " process_time " << getTimeStamp() - one_start 
    << std::endl;
}



extern "C" {
  void init_core(bool saving_whole) { 
    init(saving_whole); 
    printf("the length of union_table is %u\n", 0xC00000000/sizeof(dfsan_label_info));
    __z3_solver.set("timeout", 10000U);
    memset(pp_map, 0, kMapSize);
  }

  uint32_t run_solver(int shmid, uint32_t pipefd) {
    return solve(shmid, pipefd);
  }

  bool try_dry_run(int32_t round, uint32_t seed_id) {
    return dry_run(round, seed_id);
  }

  void wait_ce() {
    sem_wait(semace);
  }

  void post_gra() {
    sem_post(semagra);
  }

  void get_next_input(unsigned char* input, uint64_t *addr, uint64_t *ctx, 
      uint32_t *order, uint32_t *fid, uint64_t *direction, size_t size) {
    //std::pair<uint32_t, std::unordered_map<uint32_t, uint8_t>> item;
    RGDSolution item;
    //if (solution_queue.size_approx() % 1000 == 0 && solution_queue.size_approx() > 0)
    // printf("get_next_loop and queue size is %u\n", solution_queue.size_approx());
    queue_mutex.lock();
    //asert(!solutio_queue.empty());
    if(!solution_queue.empty()) {
      item = solution_queue.front(); 
      solution_queue.pop_front();
      for(auto it = item.sol.begin(); it != item.sol.end(); ++it) {
        if (it->first < size)
          input[it->first] = it->second;
      }
      *addr = item.addr;
      *ctx = item.ctx;
      *order = item.order;
      *fid = item.fid;
      *direction = item.direction;
    }
    queue_mutex.unlock();
  }

  uint32_t get_next_input_id() {
    //std::pair<uint32_t, std::unordered_map<uint32_t, uint8_t>> item;
    RGDSolution item;
    //if (solution_queue.size_approx() % 1000 == 0 && solution_queue.size_approx() > 0)
    // printf("get_next_loop and queue size is %u\n", solution_queue.size_approx());
    queue_mutex.lock();
    if(!solution_queue.empty()) {
      item = solution_queue.front(); 
      queue_mutex.unlock();
      return item.fid;
    } else {
      queue_mutex.unlock();
      // no next input
      return UINTMAX_MAX; 
    }
  }

};

