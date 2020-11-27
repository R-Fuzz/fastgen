#[link(name = "gd")]
#[link(name = "protobuf")]
#[link(name = "LLVM")]
#[link(name = "stdc++")]
extern {
   pub fn submit_task(input: *const u8, input_length: u32);
   pub fn init_core();
   pub fn aggregate_results();
}
