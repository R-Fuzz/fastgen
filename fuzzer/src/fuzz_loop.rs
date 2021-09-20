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
use std::os::unix::io::{FromRawFd, IntoRawFd, RawFd};
use nix::unistd::pipe;
use nix::unistd::close;

pub static mut FLIPPED: u32 = 0;
pub static mut ALL: u32 = 0;
pub static mut REACHED: u32 = 0;
pub static mut NOT_REACHED: u32 = 0;

pub fn dispatcher(table: &UnionTable,
    branch_gencount: Arc<RwLock<HashMap<(u64,u64,u32,u64), u32>>>,
    branch_hitcount: Arc<RwLock<HashMap<(u64,u64,u32,u64), u32>>>,
    buf: &Vec<u8>, id: RawFd) {

  let (labels,mut memcmp_data) = read_pipe(id);
  scan_nested_tasks(&labels, &mut memcmp_data, table, config::MAX_INPUT_LEN, &branch_gencount, &branch_hitcount, buf);
}

//check the status
pub fn branch_verifier(addr: u64, ctx: u64, 
    order: u32, direction: u64, fid: u32, id: RawFd) {
  let mut status = 4; // not reached
  let (labels,mut memcmp_data) = read_pipe(id);

  for label in labels {
    if label.6 == 2 {
      memcmp_data.pop_front().unwrap();
      continue;
    }
    if label.6 == 0 {
      if label.3 == addr && label.4 == ctx && label.5 == order {
        status = 2; //reached
        if label.2 == 1-direction {
          status = 1; //flipped
        }
        break;
      }
    }
  }

  unsafe {
    ALL += 1;
    if status == 2 {
      REACHED += 1;
    } else if status == 1 {
      FLIPPED += 1;
      insert_flip(addr, ctx, direction, order);
    } else if status == 4 {
      NOT_REACHED += 1;
    }
    if ALL % 100 == 0 {
      info!("verify ({},{},{},{},{}), status {}, flipped/reached/not_reached/all: {}/{}/{}/{}", addr,ctx,order,direction,fid, status, FLIPPED, REACHED, NOT_REACHED, ALL);
    }
  }
}



pub fn grading_loop(
    running: Arc<AtomicBool>,
    cmd_opt: CommandOpt,
    depot: Arc<Depot>,
    global_branches: Arc<GlobalBranches>,
    branch_gencount: Arc<RwLock<HashMap<(u64,u64,u32,u64),u32>>>,
    branch_solcount: Arc<RwLock<HashMap<(u64,u64,u32,u64),u32>>>,
    ) {

  let shmid =  unsafe {
    libc::shmget(
        libc::IPC_PRIVATE,
        0xc00000000,
        0o644 | libc::IPC_CREAT | libc::SHM_NORESERVE
        )
  };

  let mut executor = Executor::new(
      cmd_opt,
      global_branches,
      depot.clone(),
      shmid,
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
    //let mut buf: Vec<u8> = Vec::with_capacity(config::MAX_INPUT_LEN);
    //buf.resize(config::MAX_INPUT_LEN, 0);
    let mut addr: u64 = 0;
    let mut ctx: u64 = 0;
    let mut order: u32 = 0;
    let mut fid: u32 = 0;
    let mut direction: u64 = 0;
    let mut bid: u32 = 0;
    let mut sctx: u32 = 0;
    let mut flipped = 0;
    let mut sol_conds = 0;
    let mut all_conds = 0;
    let mut reached = 0;
    let mut not_reached = 0;
    let mut is_cmp = false;

    let mut blacklist: HashSet<u64> = HashSet::new();

    while running.load(Ordering::Relaxed) {
      let id = unsafe { get_next_input_id() };
      if id != std::u32::MAX {
        let mut buf: Vec<u8> = depot.get_input_buf(id as usize);
        unsafe { get_next_input(buf.as_mut_ptr(), &mut addr, &mut ctx, &mut order, &mut fid, &mut direction, &mut bid, &mut sctx, &mut is_cmp, buf.len()) };
        let new_path = executor.run_sync_with_cond(&buf, bid, sctx, order);
/*
        let direction_out = executor.get_cond();
        if (direction_out == 0 && direction == 1) || (direction_out == 1 && direction == 0) {
          flipped += 1;
          sol_conds += 1;
          info!("flipped/reached/not_reached/sol_cons {}/{}/{}/{} {}", flipped, reached, not_reached, sol_conds, addr);
          unsafe { insert_flip(addr, ctx, direction, order); }
        } else if (direction ==0 || direction == 1) && is_cmp  {
          if !blacklist.contains(&addr) {
            info!("not flipped {}, direction {}, direction_out {}, bid {} sctx {}", addr, direction, direction_out, bid, sctx);
            //std::process::exit(0);
          } 
          if (direction_out != std::u32::MAX) {
            reached += 1;
          } else {
          info!("not reached in tain verifier addr is {} bid {} sctx {} order is {}", addr, bid, sctx, order);
            not_reached += 1;
          }
          sol_conds += 1;
          info!("flipped/reached/not_reached/sol_cons {}/{}/{}/{} {}", flipped, reached, not_reached, sol_conds, addr);
        }

        all_conds += 1;
        info!("all_conds {}", all_conds);
*/
        if is_cmp {
          let (read_end, write_end) = pipe().unwrap();
          let handle = thread::spawn(move || {
              branch_verifier(addr, ctx,order,direction,fid,read_end);
              });

          let mut child = executor.track(0, &buf,write_end);
          close(write_end);

          if handle.join().is_err() {
            error!("Error happened in listening thread for branch verifier");
          }

          match child.try_wait() {
            Ok(Some(status)) => (),
              Ok(None) => {
                child.kill();
                let res = child.wait();
              }
            Err(e) => println!("error attempting to wait: {}", e),
          }
        }


        let mut solcount = 1;
        if addr != 0 && branch_solcount.read().unwrap().contains_key(&(addr, ctx, order,direction)) {
          solcount = *branch_solcount.read().unwrap().get(&(addr,ctx, order,direction)).unwrap();
          solcount += 1;
          //info!("gencount is {}",count);
        }
        branch_solcount.write().unwrap().insert((addr,ctx,order,direction), solcount);
        if new_path.0 {
          info!("grading input derived from on input {} by flipping branch@ {:#01x} ctx {:#01x} order {} direction {} bid {} sctx {}, it is a new input {}, saved as input #{}", 
                fid, addr, ctx, order, direction, bid, sctx, new_path.0, new_path.1);
          let mut count = 1;
          if addr != 0 && branch_gencount.read().unwrap().contains_key(&(addr, ctx, order,direction)) {
            count = *branch_gencount.read().unwrap().get(&(addr,ctx, order,direction)).unwrap();
            count += 1;
            //info!("gencount is {}",count);
          }
          branch_gencount.write().unwrap().insert((addr,ctx,order,direction), count);
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

pub fn constraint_solver(shmid: i32, pipe: RawFd) {
  unsafe { run_solver(shmid, pipe) };
}

//fuzz loop with parsing in C++
pub fn fuzz_loop(
    running: Arc<AtomicBool>,
    cmd_opt: CommandOpt,
    depot: Arc<Depot>,
    global_branches: Arc<GlobalBranches>,
    branch_gencount: Arc<RwLock<HashMap<(u64,u64,u32,u64),u32>>>,
    branch_solcount: Arc<RwLock<HashMap<(u64,u64,u32,u64),u32>>>,
    ) {

  let mut id: usize = 0;
  let executor_id = cmd_opt.id;

  let shmid =  
    unsafe {
      libc::shmget(
          libc::IPC_PRIVATE,
          0xc00000000,
          0o644 | libc::IPC_CREAT | libc::SHM_NORESERVE
          )
    };

  info!("start fuzz loop with shmid {}",shmid);

  //the executor to run the frontend
  let mut executor = Executor::new(
      cmd_opt,
      global_branches,
      depot.clone(),
      shmid,
  );

  while running.load(Ordering::Relaxed) {
    if id < depot.get_num_inputs() {

      let (read_end, write_end) = pipe().unwrap();
     // let handle = thread::spawn(move || {
      //    constraint_solver(shmid, read_end);
       //   });

      let t_start = time::Instant::now();

      let buf = depot.get_input_buf(id);
      let mut child = executor.track(id, &buf, write_end);
      close(write_end);
/*
      if handle.join().is_err() {
        error!("Error happened in listening thread!");
      }
*/
      constraint_solver(shmid, read_end);
      info!("Done solving {}", id);
      close(read_end);

      //let timeout = time::Duration::from_secs(90);
      //child.wait_timeout(timeout);
      match child.try_wait() {
    	Ok(Some(status)) => println!("exited with: {}", status),
    	Ok(None) => {
        	println!("status not ready yet, let's really wait");
		child.kill();
        	let res = child.wait();
        	println!("result: {:?}", res);
    	}
        Err(e) => println!("error attempting to wait: {}", e),
     }
    


      let used_t1 = t_start.elapsed();
      let used_us1 = (used_t1.as_secs() as u32 * 1000_000) + used_t1.subsec_nanos() / 1_000;
      trace!("track time {}", used_us1);
      id = id + 1;
    } else {
      if config::RUNAFL {
        info!("run afl mutator");
        if let mut buf = depot.get_input_buf(depot.next_random()) {
          run_afl_mutator(&mut executor,&mut buf);
        }
        thread::sleep(time::Duration::from_millis(10));
      } else {
        thread::sleep(time::Duration::from_secs(1));
      }
    }
  }
}


/*
pub fn fuzz_loop(
    running: Arc<AtomicBool>,
    cmd_opt: CommandOpt,
    depot: Arc<Depot>,
    global_branches: Arc<GlobalBranches>,
    branch_gencount: Arc<RwLock<HashMap<(u64,u64,u32,u64),u32>>>,
    branch_solcount: Arc<RwLock<HashMap<(u64,u64,u32,u64),u32>>>,
    ) {

  let mut id: usize = 0;
  let executor_id = cmd_opt.id;

  let shmid =  
    unsafe {
      libc::shmget(
          libc::IPC_PRIVATE,
          0xc00000000,
          0o644 | libc::IPC_CREAT | libc::SHM_NORESERVE
          )
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
  let branch_hitcount = Arc::new(RwLock::new(HashMap::<(u64,u64,u32,u64), u32>::new()));
  let mut branch_quota = HashMap::<(u64,u64,u32), u32>::new();

  let mut no_more_seeds = 0;
  while running.load(Ordering::Relaxed) {
    if id < depot.get_num_inputs() {
      //thread::sleep(time::Duration::from_millis(10));
      let buf = depot.get_input_buf(id);
      let buf_cloned = buf.clone();
      //let path = depot.get_input_path(id).to_str().unwrap().to_owned();
      let gbranch_hitcount = branch_hitcount.clone();
      let gbranch_gencount = branch_gencount.clone();

      let (read_end, write_end) = pipe().unwrap();
      let handle = thread::spawn(move || {
          dispatcher(table, gbranch_gencount, gbranch_hitcount, &buf_cloned, read_end);
          });

      let t_start = time::Instant::now();

      executor.track(id, &buf, write_end);
      close(write_end);

      if handle.join().is_err() {
        error!("Error happened in listening thread!");
      }
      close(read_end);


      let used_t1 = t_start.elapsed();
      let used_us1 = (used_t1.as_secs() as u32 * 1000_000) + used_t1.subsec_nanos() / 1_000;
      trace!("track time {}", used_us1);
      id = id + 1;
    } else {
      //let mut buf = depot.get_input_buf(depot.next_random());
      //run_afl_mutator(&mut executor,&mut buf);
      thread::sleep(time::Duration::from_secs(1));
      //break;
    }
  }
}
*/


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
