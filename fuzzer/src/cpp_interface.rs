#[link(name = "gd")]
#[link(name = "protobuf")]
#[link(name = "LLVM")]
#[link(name = "stdc++")]
extern {
  pub fn submit_task(input: *const u8, input_length: u32, expect_future: bool);
  pub fn init_core(save_whole: bool, use_codecache: bool);
  pub fn aggregate_results();
  pub fn get_input_buf(input: *mut u8);
  pub fn get_next_input(input: *mut u8, addr: *mut u64, addr: *mut u64) -> u32;
  pub fn fini_core();
}
