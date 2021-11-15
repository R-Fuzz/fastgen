use crate::{
  branches::GlobalBranches, command::CommandOpt, depot::Depot,
    executor::Executor,
};
use std::sync::{
  atomic::{AtomicBool, Ordering},
    Arc, RwLock, Mutex,
};

use std::time;
use std::thread;

use protobuf::Message;
use crate::fifo::*;
use crate::cpp_interface::*;
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
use crate::z3solver::solve;
use blockingqueue::BlockingQueue;
use crate::solution::*;
use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};

pub static mut FLIPPED: u32 = 0;
pub static mut ALL: u32 = 0;
pub static mut REACHED: u32 = 0;
pub static mut NOT_REACHED: u32 = 0;



//check the status
/*
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
    //if ALL % 100 == 0 {
    info!("verify ({},{},{},{},{}), status {}, flipped/reached/not_reached/all: {}/{}/{}/{}", addr,ctx,order,direction,fid, status, FLIPPED, REACHED, NOT_REACHED, ALL);
    // }
  }
}
*/



pub fn grading_loop(
    running: Arc<AtomicBool>,
    cmd_opt: CommandOpt,
    depot: Arc<Depot>,
    global_branches: Arc<GlobalBranches>,
    branch_gencount: Arc<RwLock<HashMap<(u64,u64,u32,u64),u32>>>,
    branch_fliplist: Arc<RwLock<HashSet<(u64,u64,u32,u64)>>>,
    forklock: Arc<Mutex<u32>>,
    solution_queue: BlockingQueue<Solution>,
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
      true,
      forklock.clone(),
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
      if let Some(buf) = buf {
        //info!("grading {:?} and length is {}", &fpath, &buf.len());
        //executor.run_norun(&buf);
        let new_path = executor.run_sync(&buf);
        info!("grading input {:?} it is a new input {}, saved as input {}", fpath, new_path.0, new_path.1);
        //std::fs::remove_file(fpath).unwrap();
        fid = fid + 1;
      }
    }
  } else {
    let mut grade_count = 0;
    //let mut buf: Vec<u8> = Vec::with_capacity(config::MAX_INPUT_LEN);
    //buf.resize(config::MAX_INPUT_LEN, 0);
    let mut addr: u64 = 0;
    //calling context
    let mut ctx: u64 = 0;
    //local count
    let mut order: u32 = 0;
    let mut fid: u32 = 0;
    //direction
    let mut direction: u64 = 0;
    //branch ID
    let mut bid: u32 = 0;
    //calling context, instrumented using Angora way
    let mut sctx: u32 = 0;
    let mut flipped = 0;
    //solved conditionns
    let mut sol_conds = 0;
    let mut reached = 0;
    let mut not_reached = 0;
    //conditional branch or switch-case
    let mut is_cmp = false;
    // number of solutions saved
    let mut saved = 0;
    // preciate for trace_cmp, should be BVEQ
    let mut predicate = 0;
    //the target case for switch-case
    let mut target_cond = 0;
    //the hash for the constraint
    let mut cons_hash = 0;

    let mut flipped_hashes: HashSet<u32> = HashSet::new();
    let mut notflipped_hashes: HashSet<u32> = HashSet::new();
    let mut notreached_hashes: HashSet<u32> = HashSet::new();

    while running.load(Ordering::Relaxed) {
      let sol = solution_queue.pop();
      //let id = unsafe { get_next_input_id() };
      //if id != std::u32::MAX {
      if let Some(mut buf) =  depot.get_input_buf(sol.fid as usize) {
        //unsafe { get_next_input(buf.as_mut_ptr(), &mut addr, &mut ctx, &mut order, 
        //     &mut fid, &mut direction, &mut bid, &mut sctx, 
        //    &mut is_cmp, &mut predicate, &mut target_cond, &mut cons_hash, buf.len()) };
        direction = sol.direction;
        bid = sol.bid;
        sctx = sol.sctx;
        order = sol.order;
        is_cmp = sol.is_cmp;
        target_cond = sol.target_cond;
        predicate = sol.predicate;
        fid = sol.fid;
        let mut_buf = mutate(buf, &sol.sol, sol.field_index, sol.field_size);
        let new_path = executor.run_sync_with_cond(&mut_buf, bid, sctx, order);

        let direction_out = executor.get_cond();
        if (direction_out == 0 && direction == 1) || (direction_out == 1 && direction == 0) {
          flipped += 1;
          branch_fliplist.write().unwrap().insert((sol.addr,sol.ctx,sol.order,sol.direction));
          sol_conds += 1;
          flipped_hashes.insert(cons_hash);
         // unsafe { insert_flip(addr, ctx, direction, order); }
        } else if predicate == 0 && is_cmp  {
          if (direction_out != std::u64::MAX) {
            reached += 1;
            notflipped_hashes.insert(cons_hash);
          } else {
            not_reached += 1;
            notreached_hashes.insert(cons_hash);
          }
          sol_conds += 1;
        } else if is_cmp && predicate !=0 {
          sol_conds += 1;
          if (direction_out == std::u64::MAX) {
            not_reached += 1;
            notreached_hashes.insert(cons_hash);
          }
          if direction == 0 {
            if direction_out == target_cond as u64 {
              flipped += 1;
              branch_fliplist.write().unwrap().insert((sol.addr,sol.ctx,sol.order,sol.direction));
              flipped_hashes.insert(cons_hash);
             // unsafe { insert_flip(addr, ctx, direction, order); }
            } else if direction_out != std::u64::MAX {
              reached += 1;
              notflipped_hashes.insert(cons_hash);
            }
          }
          if direction == 1 {
            if direction_out != target_cond as u64 {
              flipped += 1;
              branch_fliplist.write().unwrap().insert((sol.addr,sol.ctx,sol.order,sol.direction));
              flipped_hashes.insert(cons_hash);
             // unsafe { insert_flip(addr, ctx, direction, order); }
            }
          } else if direction_out != std::u64::MAX {
            reached += 1;
            notflipped_hashes.insert(cons_hash);
          }
        }
        if new_path.0 {
          saved += 1;
        }

        info!("flipped/reached/not_reached/sol_cons/saved/flipped_hashes/notflipped_hashes/notreached_hashes {}/{}/{}/{}/{}/{}/{}/{} {}", 
            flipped, reached, not_reached, sol_conds, saved, flipped_hashes.len(), notflipped_hashes.len(), notreached_hashes.len(), addr);
        /*
           if is_cmp {
           let (mut child, read_end) = executor.track(0, &buf);

           let handle = thread::spawn(move || {
           branch_verifier(addr, ctx,order,direction,fid,read_end);
           });

           if handle.join().is_err() {
           error!("Error happened in listening thread for branch verifier");
           }
           close(read_end).map_err(|err| debug!("close read end {:?}", err)).ok();

           match child.try_wait() {
           Ok(Some(status)) => (),
           Ok(None) => {
           child.kill();
           let res = child.wait();
           }
           Err(e) => println!("error attempting to wait: {}", e),
           }
           }
         */

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
        //}
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

pub fn constraint_solver(shmid: i32, pipe: RawFd, solution_queue: BlockingQueue<Solution>, tainted_size: usize,
    branch_gencount: Arc<RwLock<HashMap<(u64,u64,u32,u64), u32>>>,
    branch_fliplist: Arc<RwLock<HashSet<(u64,u64,u32,u64)>>>,
    branch_hitcount: Arc<RwLock<HashMap<(u64,u64,u32,u64), u32>>>) {
  unsafe { solve(shmid, pipe, solution_queue, tainted_size, &branch_gencount, &branch_fliplist, &branch_hitcount); };
}

//fuzz loop with parsing in C++
pub fn fuzz_loop(
    running: Arc<AtomicBool>,
    cmd_opt: CommandOpt,
    depot: Arc<Depot>,
    global_branches: Arc<GlobalBranches>,
    branch_gencount: Arc<RwLock<HashMap<(u64,u64,u32,u64),u32>>>,
    branch_fliplist: Arc<RwLock<HashSet<(u64,u64,u32,u64)>>>,
    restart: bool,
    forklock: Arc<Mutex<u32>>,
    bq: BlockingQueue<Solution>,
    ) {

  let mut id: u32 = 0;
  let executor_id = cmd_opt.id;

  if restart {
    let progress_data = std::fs::read("ce_progress").unwrap();
    id = (&progress_data[..]).read_u32::<LittleEndian>().unwrap();
    println!("restarting scan from id {}",id);
  }

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
      true, //not grading
      forklock.clone(),
      );

  let branch_hitcount = Arc::new(RwLock::new(HashMap::<(u64,u64,u32,u64), u32>::new()));

  while running.load(Ordering::Relaxed) {
    if (id as usize) < depot.get_num_inputs() {


      let t_start = time::Instant::now();

      if let Some(buf) = depot.get_input_buf(id as usize) {
        let (mut child, read_end) = executor.track(id as usize, &buf);

        let gbranch_hitcount = branch_hitcount.clone();
        let gbranch_fliplist = branch_fliplist.clone();
        let gbranch_gencount = branch_gencount.clone();
        let solution_queue = bq.clone();
        let handle = thread::Builder::new().stack_size(64 * 1024 * 1024).spawn(move || {
            constraint_solver(shmid, read_end, solution_queue, buf.len(), gbranch_gencount, gbranch_fliplist, gbranch_hitcount);
            }).unwrap();

        if handle.join().is_err() {
          error!("Error happened in listening thread!");
        }

        //      constraint_solver(shmid, read_end);
        info!("Done solving {}", id);
        close(read_end).map_err(|err| debug!("close read end {:?}", err)).ok();

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
        let mut progress = Vec::new();
        progress.write_u32::<LittleEndian>(id).unwrap();
        std::fs::write("ce_progress", &progress).map_err(|err| println!("{:?}", err)).ok();
      }
    } else {
      if config::RUNAFL {
        info!("run afl mutator");
        if let Some(mut buf) = depot.get_input_buf(depot.next_random()) {
          run_afl_mutator(&mut executor,&mut buf);
        }
        thread::sleep(time::Duration::from_millis(10));
      } else {
        thread::sleep(time::Duration::from_secs(1));
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
