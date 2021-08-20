#[link(name = "gd")]
//#[link(name = "protobuf")]
//#[link(name = "LLVM")]
#[link(name = "stdc++")]
#[link(name = "z3")]
extern {
  pub fn submit_task(input: *const u8, input_length: u32, expect_future: bool, fresh: bool);
  pub fn submit_fmemcmp(data: *const u8, index: u64, size: u32, tid: u32, addr: u64);
  pub fn append_fid(addr: u64, ctx: u64, order: u32, direction: u64, fid: u32);
  pub fn get_queue_length() -> u32;
  pub fn init_core(save_whole: bool, use_codecache: bool);
  pub fn aggregate_results();
  pub fn get_input_buf(input: *mut u8);
  pub fn get_next_input(input: *mut u8, addr: *mut u64, ctx: *mut u64, 
        order: *mut u32, fid: *mut u32, direction: *mut u64,
        bid: *mut u32, sctx: *mut u32, size: usize);
  pub fn get_next_input_id() -> u32;
  pub fn fini_core();
  pub fn run_solver(shmid: i32, pipefd: i32);
  pub fn insert_flip(addr: u64, ctx: u64, direction: u64, order: u32);
}
