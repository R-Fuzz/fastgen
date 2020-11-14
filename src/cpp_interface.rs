#[link(name = "gd")]
#[link(name = "protobuf")]
#[link(name = "LLVM")]
#[link(name = "stdc++")]
extern {
   pub fn print_buffer(input: *const u8, input_length: u32);
}
