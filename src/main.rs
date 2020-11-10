use libc;
use protoc_rust::Customize;
#[repr(C,align(8))] 
pub struct dfsan_label_info {
  l1: u32,
  l2: u32,
  op1: u64,
  op2: u64,
  op: u16,
  size: u16,
  flags: u8,
  tree_size: u32,
  hash: u32,
  unused1: u64, //this is *expr 
  unused2: u64,
}

pub type UnionTable = [dfsan_label_info; 50331648];

fn main() {
    println!("Hello, world!");
    let id = unsafe {
      libc::shmget(
        0x1234,
        0xc00000000, 
        0644 | libc::IPC_CREAT | libc::SHM_NORESERVE
      )
    };
    let ptr = unsafe { libc::shmat(id, std::ptr::null(), 0) as *mut UnionTable};
    let loc = unsafe {&mut *ptr };
    let loc1 = &loc[42];
    println!("l1 is {:?}", loc1.l1);
}
