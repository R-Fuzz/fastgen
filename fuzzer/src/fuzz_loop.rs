use crate::{
  branches::GlobalBranches, command::CommandOpt, depot::Depot,
    executor::Executor,
};
use std::sync::{
  atomic::{AtomicBool, Ordering},
    Arc,
};
use std::time; use std::thread;

use protobuf::Message;
use crate::fifo::*;
use crate::cpp_interface::*;
use crate::track_cons::*;
use crate::union_table::*;
use crate::file::*;
use fastgen_common::config;
use std::path::{Path};

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

  let t_start = time::Instant::now();
  if config::SAVING_WHOLE {
    let mut fid = 1;
    let dirpath = Path::new("/home/cju/test");
    while running.load(Ordering::Relaxed) {
      let file_name = format!("id-{:08}", fid);
      let fpath = dirpath.join(file_name);
      if !fpath.exists() {
        continue;
      }
      trace!("grading {:?}", &fpath);
      let buf = read_from_file(&fpath);
      executor.run_sync(&buf);
      std::fs::remove_file(fpath).unwrap();
      fid = fid + 1;
    }
  } else {
    let mut grade_count = 0;
    let mut buf: Vec<u8> = Vec::with_capacity(1000);
    buf.resize(1000, 0);
    while running.load(Ordering::Relaxed) {
      let len = unsafe { get_next_input(buf.as_mut_ptr()) };
      if len != 0 {
        buf.resize(len as usize, 0);
        executor.run_sync(&buf);
        grade_count = grade_count + 1;
      }
      if grade_count % 1000 == 0 {
        let used_t1 = t_start.elapsed().as_secs() as u32;
        if used_t1 != 0 {
          warn!("Grading throughput is {}", grade_count / used_t1);
        }
      }
    }
  }
}


pub fn dispatcher(table: &UnionTable) {
  let labels = read_pipe();
  let mut tasks = Vec::new();
  scan_tasks(&labels, &mut tasks, table); 
  for task in tasks {
    let task_ser = task.write_to_bytes().unwrap();
    unsafe { submit_task(task_ser.as_ptr(), task_ser.len() as u32); }
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

  let shmid = unsafe {
    libc::shmget(
        0x1234,
        0xc00000000,
        0o644 | libc::IPC_CREAT | libc::SHM_NORESERVE
        )
  };
  let ptr = unsafe { libc::shmat(shmid, std::ptr::null(), 0) as *mut UnionTable};
  let table = unsafe { & *ptr };

  while running.load(Ordering::Relaxed) {
    if id < depot.get_num_inputs() {
      let buf = depot.get_input_buf(id);
      //let path = depot.get_input_path(id).to_str().unwrap().to_owned();

      let handle = thread::spawn(move || {
          dispatcher(table);
          });
      let t_start = time::Instant::now();
      executor.track(id, &buf);
      if handle.join().is_err() {
        error!("Error happened in listening thread!");
      }
      let used_t1 = t_start.elapsed();
      let used_us1 = (used_t1.as_secs() as u32 * 1000_000) + used_t1.subsec_nanos() / 1_000;
      trace!("track time {}", used_us1);
      id = id + 1;
    }
  }
}


#[cfg(test)]
mod tests {
  use super::*;
  use std::fs;
  use std::path::PathBuf;
  use crate::depot;
  use crate::command;
  use crate::branches;

#[test]
  fn test_pointer() {
    let mut buf: Vec<u8> = Vec::with_capacity(10);
    buf.resize(10, 0);
    unsafe { get_input_buf(buf.as_mut_ptr()); }
    println!("{}",buf[0])
  }

#[test]
  fn test_grading() {
    let angora_out_dir = PathBuf::from("output");
    let seeds_dir = PathBuf::from("input");
    let args = vec!["./objdump.fast".to_string(), "-D".to_string(), "@@".to_string()];
    fs::create_dir(&angora_out_dir).expect("Output directory has existed!");

    let cmd_opt = command::CommandOpt::new("./objdump.track", args, &angora_out_dir, 200, 1);

    let depot = Arc::new(depot::Depot::new(seeds_dir, &angora_out_dir));
  
    let global_branches = Arc::new(branches::GlobalBranches::new());

    let mut executor = Executor::new(
        cmd_opt.specify(1),
        global_branches.clone(),
        depot.clone(),
        );

    let t_start = time::Instant::now();
    let mut fid = 1;
    let dirpath = Path::new("/home/cju/test");
    let mut count = 0;
    loop {
      let file_name = format!("id-{:08}", fid);
      let fpath = dirpath.join(file_name);
      if !fpath.exists() {
        break;
      }
      trace!("grading {:?}", &fpath);
      let buf = read_from_file(&fpath);
      executor.run_sync(&buf);
      fid = fid + 1;
      count = count + 1;
    }
    let used_t1 = t_start.elapsed();
    if used_t1.as_secs() as u32 !=0  {
      println!("throught put is {}", count / used_t1.as_secs() as u32);
    }
  }
}
