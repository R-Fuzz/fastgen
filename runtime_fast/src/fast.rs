use super::{forkcli, shm_branches, shm_conds};
use std::ops::DerefMut;
use std::sync::Once;

static START: Once = Once::new();

#[ctor]
fn fast_init() {
  START.call_once(|| {
      shm_branches::map_branch_counting_shm();
      forkcli::start_forkcli();
      });
}

#[no_mangle]
pub extern "C" fn __angora_trace_cmp(
    cmpid: u32,
    context: u32,
    condition: u64,
    ) -> u64 {
  let mut conds = shm_conds::SHM_CONDS.lock().expect("SHM mutex poisoned.");
  //eprintln!("_angora_trace_cmp {} {} {}", cmpid, context, condition);
  match conds.deref_mut() {
    &mut Some(ref mut c) => {
      if c.check_match(cmpid, context) {
        return c.update_cmp(condition);
      }
    }
    _ => {
      //eprintln!("no conds");
    }
  }
  condition
}

#[no_mangle]
pub extern "C" fn __grade_trace_cond(
    cmpid: u32,
    context: u32,
    condition: u64,
    ) -> u64 {
  
  let mut conds = shm_conds::SHM_CONDS.lock().expect("SHM mutex poisoned.");
  //eprintln!("_grade_trace_cond {} {} {}", cmpid, context, condition);
  match conds.deref_mut() {
    &mut Some(ref mut c) => {
      if c.check_match(cmpid, context) {
        return c.update_cmp(condition);
      }
    }
    _ => {
      //eprintln!("no conds");
    }
  }
  condition

}

#[no_mangle]
pub extern "C" fn __grade_trace_switch(
    cmpid: u32,
    context: u32,
    condition: u64,
    ) -> u64 {
  
  let mut conds = shm_conds::SHM_CONDS.lock().expect("SHM mutex poisoned.");
  //eprintln!("_grade_trace_switch {} {} {}",cmpid, context, condition);
  match conds.deref_mut() {
    &mut Some(ref mut c) => {
      if c.check_match(cmpid, context) {
        return c.update_cmp(condition);
      }
    }
    _ => {
      //eprintln!("no conds");
    }
  }
  condition

}

#[no_mangle]
pub extern "C" fn __angora_trace_switch(
    cmpid: u32,
    context: u32,
    condition: u64,
    ) -> u64 {
  eprintln!("_angora_trace_switch {} {}", cmpid, context);
  condition
}
