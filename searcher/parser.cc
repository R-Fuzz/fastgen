#include "task.h"
#include "rgd.pb.h"
#include "rgd_op.h"
#include "jit.h"
using namespace rgd;

static std::atomic_uint64_t uuid;


static void append_meta(std::shared_ptr<Cons> cons, const Constraint* c) {
  for (auto amap : c->meta().map()) {
    cons->local_map.insert({amap.k(), amap.v()});
  }
  for (auto aarg : c->meta().args()) {
    cons->input_args.push_back({aarg.isinput(), aarg.v()});
  }
  for (auto ainput : c->meta().inputs()) {
    cons->inputs.insert({ainput.offset(), ainput.iv()});
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
    if (c.node().kind() != rgd::Constant) {
      append_meta(cons, &c);
      uint64_t id = uuid.fetch_add(1, std::memory_order_relaxed);
      addFunction(&c.node(), cons->local_map, id);
    } else {
      cons->is_const = true;
    }
  }

}
