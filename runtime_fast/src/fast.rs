use super::{forkcli, shm_branches};

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
  condition: u32,
  cmpid: u32,
  context: u32,
) -> u32 {
  eprintln!("_angora_trace_cmp {} {}", cmpid, context);
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
