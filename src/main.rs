use libc;
//use quickgen::rgd::*;
use quickgen::union_table::*;
use quickgen::track_cons::*;
use protobuf::Message;


#[link(name = "gd")]
#[link(name = "protobuf")]
#[link(name = "stdc++")]
extern {
    fn print_buffer(input: *const u8, input_length: u32);
}

fn main() {
  let id = unsafe {
    libc::shmget(
        0x1234,
        0xc00000000, 
        0644 | libc::IPC_CREAT | libc::SHM_NORESERVE
        )
  };
  let ptr = unsafe { libc::shmat(id, std::ptr::null(), 0) as *mut UnionTable};
  let table = unsafe { & *ptr };

  let mut tasks = Vec::new();
  scan_tasks(&mut tasks, table); 
  for task in tasks {
    let task_ser = task.write_to_bytes().unwrap();
    unsafe { print_buffer(task_ser.as_ptr(), task_ser.len() as u32); }
  }
  
}
