use crate::{
  branches::GlobalBranches, command::CommandOpt, depot::Depot,
    executor::Executor,
};
use rand::prelude::*;
use std::sync::{
  atomic::{AtomicBool, Ordering},
    Arc, RwLock,
};
use std::time;
use std::thread;
use crate::fifo::*;

use protobuf::Message;
use crate::fifo::*;
use crate::util::*;
use crate::cpp_interface::*;
use crate::track_cons::*;
use crate::union_table::*;

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
      print_task(&task);
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
    //trace!("runninng one epoch and number is {}", depot.get_num_inputs());
    thread::sleep(time::Duration::from_secs(1));
  }
}
