use crate::{
  branches::GlobalBranches, command::CommandOpt, depot::Depot,
    executor::Executor,
};
use std::sync::{
  atomic::{AtomicBool, Ordering},
    Arc, RwLock,
};

use std::time;
use std::thread;

use protobuf::Message;
use crate::fifo::*;
use crate::cpp_interface::*;
use crate::track_cons::*;
use crate::union_table::*;
use crate::file::*;
use crate::afl::*;
use fastgen_common::config;
use std::path::{Path};
use crate::rgd::*;
use std::collections::HashSet;
use std::collections::HashMap;
//use crate::util::*;
use wait_timeout::ChildExt;

pub fn grading_loop(
    running: Arc<AtomicBool>,
    cmd_opt: CommandOpt,
    depot: Arc<Depot>,
    global_branches: Arc<GlobalBranches>,
    branch_gencount: Arc<RwLock<HashMap<(u64,u64,u32),u32>>>,
    ) {
  let mut executor = Executor::new(
      cmd_opt,
      global_branches,
      depot.clone(),
      0,
      );

  //let branch_gencount = Arc::new(RwLock::new(HashMap::<(u64,u64,u32), u32>::new()));
  let t_start = time::Instant::now();
  if config::SAVING_WHOLE {
    let mut fid = 0;
    let dirpath = Path::new("/home/cju/test");
    while running.load(Ordering::Relaxed) {
      let file_name = format!("id-{:08}", fid);
      let fpath = dirpath.join(file_name);
      if !fpath.exists() {
        continue;
      }
      //wait untill file fully flushed
      thread::sleep(time::Duration::from_millis(1));
      let buf = read_from_file(&fpath);
      if buf.len() == 0 {
        continue;
      }
      //info!("grading {:?} and length is {}", &fpath, &buf.len());
      //executor.run_norun(&buf);
      let new_path = executor.run_sync(&buf);
      info!("grading input {:?} it is a new input {}, saved as input {}", fpath, new_path.0, new_path.1);
      //std::fs::remove_file(fpath).unwrap();
      fid = fid + 1;
    }
  } else {
    let mut grade_count = 0;
    let mut buf: Vec<u8> = Vec::with_capacity(config::MAX_INPUT_LEN);
    buf.resize(config::MAX_INPUT_LEN, 0);
    let mut addr: u64 = 0;
    let mut ctx: u64 = 0;
    let mut order: u32 = 0;
    let mut fid: u32 = 0;
    while running.load(Ordering::Relaxed) {
      let len = unsafe { get_next_input(buf.as_mut_ptr(), &mut addr, &mut ctx, &mut order, &mut fid) };
      if len != 0 {
        buf.resize(len as usize, 0);
        let new_path = executor.run_sync(&buf);
        if new_path.0 {
          info!("grading input derived from on input {} by flipping branch@ {:#01x} ctx {:#01x} order {}, it is a new input {}, saved as input #{}", fid, addr, ctx, order, new_path.0, new_path.1);
          let mut count = 1;
          if addr != 0 && branch_gencount.read().unwrap().contains_key(&(addr, ctx, order)) {
            count = *branch_gencount.read().unwrap().get(&(addr,ctx, order)).unwrap();
            count += 1;
            //info!("gencount is {}",count);
          }
          branch_gencount.write().unwrap().insert((addr,ctx,order), count);
          //info!("next input addr is {:} ctx is {}",addr,ctx);
        }
        grade_count = grade_count + 1;
      }
      if grade_count % 1000 == 0 {
        let used_t1 = t_start.elapsed().as_secs() as u32;
        if used_t1 != 0 {
       //   warn!("Grading throughput is {}", grade_count / used_t1);
        }
      }
    }
  }
}


pub fn dispatcher(table: &UnionTable, global_tasks: Arc<RwLock<Vec<SearchTask>>>,
    dedup: Arc<RwLock<HashSet<(u64,u64,u32, u64)>>>,
    branch_hitcount: Arc<RwLock<HashMap<(u64,u64,u32), u32>>>,
    buf: &Vec<u8>, id: usize) {

  let (labels,mut memcmp_data) = read_pipe(id);
  scan_nested_tasks(&labels, &mut memcmp_data, table, config::MAX_INPUT_LEN, &dedup, &branch_hitcount, &global_tasks, buf);
}

pub fn fuzz_loop(
    running: Arc<AtomicBool>,
    cmd_opt: CommandOpt,
    depot: Arc<Depot>,
    global_branches: Arc<GlobalBranches>,
    branch_gencount: Arc<RwLock<HashMap<(u64,u64,u32),u32>>>,
    ) {

  let mut id: usize = 0;
  let executor_id = cmd_opt.id;

  let shmid = match executor_id { 
    2 => unsafe {
    libc::shmget(
        0x1234,
        0xc00000000,
        0o644 | libc::IPC_CREAT | libc::SHM_NORESERVE
        )
    },
    3 => unsafe {
    libc::shmget(
        0x2468,
        0xc00000000,
        0o644 | libc::IPC_CREAT | libc::SHM_NORESERVE
        )
    },
    _ => 0,
  };

  info!("start fuzz loop with shmid {}",shmid);

  let mut executor = Executor::new(
      cmd_opt,
      global_branches,
      depot.clone(),
      shmid,
      );

  let ptr = unsafe { libc::shmat(shmid, std::ptr::null(), 0) as *mut UnionTable};
  let table = unsafe { & *ptr };
  let global_tasks = Arc::new(RwLock::new(Vec::<SearchTask>::new()));
  let dedup = Arc::new(RwLock::new(HashSet::<(u64,u64,u32, u64)>::new()));
  let branch_hitcount = Arc::new(RwLock::new(HashMap::<(u64,u64,u32), u32>::new()));
  let mut branch_quota = HashMap::<(u64,u64,u32), u32>::new();

  let mut no_more_seeds = 0;
  while running.load(Ordering::Relaxed) {
    if id < depot.get_num_inputs() {
      //thread::sleep(time::Duration::from_millis(10));
      let buf = depot.get_input_buf(id);
      let buf_cloned = buf.clone();
      //let path = depot.get_input_path(id).to_str().unwrap().to_owned();
      let gtasks = global_tasks.clone();
      let gdedup = dedup.clone();
      let gbranch_hitcount = branch_hitcount.clone();

      let handle = thread::spawn(move || {
          dispatcher(table, gtasks, gdedup, gbranch_hitcount, &buf_cloned, executor_id);
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
    } else {
      let mut buf = depot.get_input_buf(depot.next_random());
      run_afl_mutator(&mut executor,&mut buf);
      if !config::SAMPLING {
        continue;
      }
      no_more_seeds = no_more_seeds + 1;
      if no_more_seeds > 10 {
        no_more_seeds = 0;
        info!("Rerun all {} tasks", global_tasks.read().unwrap().len());
        let cloned_branchhit: HashMap<(u64,u64,u32),u32> = branch_hitcount.read().unwrap().clone();
        let cloned_branchgen: HashMap<(u64,u64,u32),u32> = branch_gencount.read().unwrap().clone();
        let mut hitcount_vec: Vec<(&(u64,u64,u32), &u32)> = cloned_branchhit.iter().collect();
        let mut gencount_vec: Vec<(&(u64,u64,u32), &u32)> = cloned_branchgen.iter().collect();
        hitcount_vec.sort_by(|a, b| a.1.cmp(b.1));
        gencount_vec.sort_by(|a, b| b.1.cmp(a.1));
/*
        for item in hitcount_vec {
          println!("Most frequently hit branch are {:?}, count is {}", item.0, item.1);
        }

        warn!("gencount items is {}", gencount_vec.len());

        for item in gencount_vec {
          println!("Most frequently gen branch are {:?}, count is {}", item.0, item.1);
        }
*/

        let mut scheduled_count = 0;
        for task in global_tasks.read().unwrap().iter() {
          let if_quota;
          let mut quota;
          if branch_quota.contains_key(&(task.get_addr(), task.get_ctx(), task.get_order())) {
            quota = *branch_quota.get(&(task.get_addr(), task.get_ctx(), task.get_order())).unwrap(); 
            if quota > 0 {
              if_quota = true;
              quota = quota - 1;
            } else {
              if_quota = false;
            }
          } else {
            quota = 9;
            if_quota = true;
          }
          branch_quota.insert((task.get_addr(), task.get_ctx(), task.get_order()), quota);

          let hitcount = match cloned_branchhit.get(&(task.get_addr(), task.get_ctx(), task.get_order())) {
            Some(&x) => x,
            None => 0,
          };

          let gencount = match cloned_branchgen.get(&(task.get_addr(), task.get_ctx(), task.get_order())) {
            Some(&x) => x,
            None => 0,
          };
          if if_quota || (!if_quota && (gencount > 1 || hitcount < 5)) {
          //if hitcount < 5 || gencount > 1 {
            scheduled_count += 1;
            let task_ser = task.write_to_bytes().unwrap();
            unsafe { submit_task(task_ser.as_ptr(), task_ser.len() as u32, false, false); }
          }
        }
        info!("scheduled_count {}", scheduled_count);
        thread::sleep(time::Duration::from_secs(scheduled_count/1000));
        //break;
      }
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
    let args = vec!["./size.fast".to_string(), "@@".to_string()];
    fs::create_dir(&angora_out_dir).expect("Output directory has existed!");

    let cmd_opt = command::CommandOpt::new("./size.track", args, &angora_out_dir, 200, 1);

    let depot = Arc::new(depot::Depot::new(seeds_dir, &angora_out_dir));

    let global_branches = Arc::new(branches::GlobalBranches::new());

    let mut executor = Executor::new(
        cmd_opt.specify(1),
        global_branches.clone(),
        depot.clone(),
        0);

    let t_start = time::Instant::now();
    let mut fid = 0;
    let dirpath = Path::new("/home/cju/test");
    let mut count = 0;
    loop {
      let file_name = format!("id-{:08}", fid);
      println!("file name is {:?}",file_name);
      let fpath = dirpath.join(file_name);
      if !fpath.exists() {
        break;
      }
      let buf = read_from_file(&fpath);
      println!("grading {:?}, len {}", &fpath,buf.len() );
      let newpath = executor.run_sync(&buf);
      println!("grading {}",newpath.0);
      fid = fid + 1;
      count = count + 1;
    }
    let used_t1 = t_start.elapsed();
    if used_t1.as_secs() as u32 !=0  {
      println!("throught put is {}", count / used_t1.as_secs() as u32);
    }
  }
}
