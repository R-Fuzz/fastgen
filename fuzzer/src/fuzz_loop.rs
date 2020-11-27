use crate::{
  branches::GlobalBranches, command::CommandOpt, depot::Depot,
    executor::Executor,
};
use rand::prelude::*;
use std::sync::{
  atomic::{AtomicBool, Ordering},
    Arc, RwLock,
};
use std::time; use std::thread;
use crate::fifo::*;

use protobuf::Message;
use crate::fifo::*;
use crate::util::*;
use crate::cpp_interface::*;
use crate::track_cons::*;
use crate::union_table::*;
use crate::file::*;
use std::path::Path;

pub fn grading_loop(
    running: Arc<AtomicBool>,
    cmd_opt: CommandOpt,
    depot: Arc<Depot>,
    global_branches: Arc<GlobalBranches>,
    ) {
  let mut executor = Executor::new(
      cmd_opt,
      global_branches,
      depot.clone(),
      );

  let mut fid: u64 = 0;
  loop {
    /*
       let dirpath = Path::new("/home/cju/test");
       let file_name = format!("id-{:08}", fid);
       let fpath = dirpath.join(file_name);
       if !fpath.exists() {
       continue;
       }
       trace!("grading {:?}", &fpath);
       let buf = read_from_file(&fpath);
       executor.run_sync(&buf);
       std::fs::remove_file(fpath);
       fid = fid + 1;
     */
    let mut buf: Vec<u8> = Vec::with_capacity(1000);
    buf.resize(1000, 0);
    let len = unsafe { get_next_input(buf.as_mut_ptr()) };
    if (len != 0) {
      buf.resize(len as usize, 0);
      executor.run_sync(&buf);
    }
  }
}

pub fn dispatcher() {
  info!("in dispatcher!!");
  loop {
    info!("read pipe!!");
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
    let labels = read_pipe();
    scan_tasks(&labels, &mut tasks, table); 
    for task in tasks {
      //print_task(&task);
      let task_ser = task.write_to_bytes().unwrap();
      unsafe { submit_task(task_ser.as_ptr(), task_ser.len() as u32); }
    }

  }
}

pub fn fuzz_loop(
    running: Arc<AtomicBool>,
    cmd_opt: CommandOpt,
    depot: Arc<Depot>,
    global_branches: Arc<GlobalBranches>,
    ) {
  let mut executor = Executor::new(
      cmd_opt,
      global_branches,
      depot.clone(),
      );
  let mut id: usize = 0;
  while running.load(Ordering::Relaxed) {
    if id < depot.get_num_inputs() {
      let buf = depot.get_input_buf(id);
      let path = depot.get_input_path(id).to_str().unwrap().to_owned();
      executor.track(id, &buf, &path);
      id = id + 1;
    }
    thread::sleep(time::Duration::from_secs(1));
  }
}


#[cfg(test)]
mod tests {
  use super::*;

#[test]
  fn test_pointer() {
    let mut buf: Vec<u8> = Vec::with_capacity(10);
    buf.resize(10, 0);
    unsafe { get_input_buf(buf.as_mut_ptr()); }
    println!("{}",buf[0])
  }
}
