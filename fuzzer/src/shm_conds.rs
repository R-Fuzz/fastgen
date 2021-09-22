use fastgen_common::{cond_stmt_base::CondStmtBase, defs, shm};
use std;
use crate::shm_conds;

pub struct ShmConds {
  pub cond: shm::SHM<CondStmtBase>,
}

impl ShmConds {
  pub fn new() -> Self {
    Self {
cond: shm::SHM::<CondStmtBase>::new(),
    }
  }

#[inline(always)]
  pub fn get_id(&self) -> i32 {
    self.cond.get_id()
  }


  pub fn clear(&mut self) {
    self.cond.cmpid = 0;
    self.cond.order = 0;
    self.cond.context = 0;
  }

  pub fn set(&mut self, cmpid: u32, ctx: u32, order: u32) {
    self.cond.cmpid = cmpid;
    self.cond.context = ctx;
    self.cond.order= order;
    self.cond.condition = std::u64::MAX;
  }

}
