#include "task.h"
#include "rgd.pb.h"
#include "rgd_op.h"
using namespace rgd;

static void append_meta(std::shared_ptr<Cons> cons, const AstNode* node, int left_right) {
  if (left_right) {
    for (auto amap : node->meta().map()) {
      cons->local_map_left.insert({amap.k(), amap.v()});
    }
    for (auto aarg : node->meta().args()) {
      cons->input_args_left.push_back({aarg.isinput(), aarg.v()});
    }
    for (auto ainput : node->meta().inputs()) {
      cons->inputs_left.insert({ainput.offset(), ainput.iv()});
    }
  } else {
    for (auto amap : node->meta().map()) {
      cons->local_map_right.insert({amap.k(), amap.v()});
    }
    for (auto aarg : node->meta().args()) {
      cons->input_args_right.push_back({aarg.isinput(), aarg.v()});
    }
    for (auto ainput : node->meta().inputs()) {
      cons->inputs_right.insert({ainput.offset(), ainput.iv()});
    }
  }
}

FUT* construct_task(SearchTask* task) {
  struct FUT *fut = new FUT();
  fut->gsol = false;
  fut->att = 0;
  fut->stopped = false;
  fut->num_minimal_optima = 0;
  for (auto c : task->constraints()) {
    std::shared_ptr<Cons> cons = std::make_shared<Cons>();
    if (c.left().kind() != rgd::Constant) {
      append_meta(cons, &c.left(), 1);
    } else {
      cons->is_left_const = true;
    }
    if (c.right().kind() != rgd::Constant) {
      append_meta(cons, &c.right(), 0);
    } else {
      cons->is_left_const = true;
    }
  }
  
}
