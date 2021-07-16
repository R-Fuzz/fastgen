use crate::{
  branches::GlobalBranches, command::CommandOpt, depot::Depot,
    executor::Executor,
};
use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use std::sync::{
  atomic::{AtomicBool, Ordering},
    Arc, RwLock,
};

use std::time;
use std::thread;

use crate::fifo::*;
use crate::cpp_interface::*;
use crate::track_cons::*;
use crate::union_table::*;
use crate::file::*;
use fastgen_common::config;
use std::path::{Path};
use std::collections::HashSet;
use std::collections::HashMap;
//use crate::util::*;
use std::os::unix::io::{RawFd};
use nix::unistd::pipe;
use nix::unistd::close;


pub fn dispatcher(table: &UnionTable,
    branch_gencount: Arc<RwLock<HashMap<(u64,u64,u32,u64), u32>>>,
    branch_hitcount: Arc<RwLock<HashMap<(u64,u64,u32,u64), u32>>>,
    buf: &Vec<u8>, id: RawFd) {

  let (labels,mut memcmp_data) = read_pipe(id);
  scan_nested_tasks(&labels, &mut memcmp_data, table, config::MAX_INPUT_LEN, &branch_gencount, &branch_hitcount, buf);
}

//check the status
pub fn branch_verifier(id: RawFd, addr: u64, ctx: u64, 
    order: u32, direction: u64, fid: u32,
    branch_solcount: Arc<RwLock<HashMap<(u64,u64,u32,u64), u32>>>) {
  let mut status = 4; // not reached
  let (labels,mut memcmp_data) = read_pipe(id);

  let mut blacklist: HashSet<u64> = HashSet::new();
  blacklist.insert(123145304593908);
  blacklist.insert(123145304594353);
  blacklist.insert(123145304595949);
  blacklist.insert(123145304594728);
  blacklist.insert(123145304669312);
  blacklist.insert(123145304669035);
  blacklist.insert(123145304660330);
  blacklist.insert(123145304602306);
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
      }
    }
  }

  println!("verify ({},{},{},{},{}), status {}", addr,ctx,order,direction,fid, status);
  if status == 4 && !blacklist.contains(&addr) && (direction == 0 || direction == 1) {
    std::process::exit(1);
  }
  let mut status_to_update = 3;
  if branch_solcount.read().unwrap().contains_key(&(addr, ctx, order,direction)) {
    status_to_update = *branch_solcount.read().unwrap().get(&(addr,ctx, order,direction)).unwrap();
  }
  if status < status_to_update {
    status_to_update = status;
  }
  branch_solcount.write().unwrap().insert((addr,ctx,order,direction), status_to_update);
}

pub fn branch_checking(
    running: Arc<AtomicBool>,
    cmd_opt: CommandOpt,
    depot: Arc<Depot>,
    global_branches: Arc<GlobalBranches>,
    branch_gencount: Arc<RwLock<HashMap<(u64,u64,u32,u64),u32>>>,
    // let's use this for the solving status
    // 1 -> flipped
    // 2 -> reached not flipped
    // 3 -> not reached
    branch_solcount: Arc<RwLock<HashMap<(u64,u64,u32,u64),u32>>>,
    ) {

  let shmid =  unsafe {
    libc::shmget(
        0x2468,
        0xc00000000,
        0o644 | libc::IPC_CREAT | libc::SHM_NORESERVE
        )
  };

  let mut executor = Executor::new(
      cmd_opt,
      global_branches,
      depot.clone(),
      shmid,
      false, //not grading
      );



  //let branch_gencount = Arc::new(RwLock::new(HashMap::<(u64,u64,u32), u32>::new()));
  let mut grade_count = 0;
  let mut addr: u64 = 0;
  let mut ctx: u64 = 0;
  let mut order: u32 = 0;
  let mut fid: u32 = 0;
  let mut direction: u64 = 0;
  //let mut veri_status: Arc<AtomicU32>;
  while running.load(Ordering::Relaxed) {
    let id = unsafe { get_next_input_id() };
    if let Some(mut buf) = depot.get_input_buf(id as usize) {
      unsafe { get_next_input(buf.as_mut_ptr(), &mut addr, &mut ctx, &mut order, &mut fid, &mut direction, buf.len()) };
      let gsol_count = branch_solcount.clone();
      //let v_status = veri_status.clone();

      let (read_end, write_end) = pipe().unwrap();
      let handle = thread::spawn(move || {
          branch_verifier(read_end, addr, ctx,order,direction,fid,gsol_count);
          });

      executor.track(0, &buf, read_end, write_end);

      if handle.join().is_err() {
        error!("Error happened in listening thread!");
      }
      close(read_end).map_err(|err| println!("{:?}", err)).ok();


      //if (veri_status.load(Ordering::Relaxed) == 4) {

      //   panic!("branch not reached");
      //}

      let new_path = executor.run_sync(&buf);
      if new_path.0 {
        info!("grading input derived from on input {} by flipping branch@ {:#01x} ctx {:#01x} order {}, it is a new input {}, saved as input #{}", fid, addr, ctx, order, new_path.0, new_path.1);
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
  let mut executor = Executor::new(
      cmd_opt,
      global_branches,
      depot.clone(),
      0,
      true, //grading
      );

  //let branch_gencount = Arc::new(RwLock::new(HashMap::<(u64,u64,u32), u32>::new()));
  let t_start = time::Instant::now();
  if config::SAVING_WHOLE {
    let mut fid = 0;
    let dirpath = Path::new("./raw_cases");
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
    let mut buf: Vec<u8> = Vec::with_capacity(config::MAX_INPUT_LEN);
    buf.resize(config::MAX_INPUT_LEN, 0);
    let mut addr: u64 = 0;
    let mut ctx: u64 = 0;
    let mut order: u32 = 0;
    let mut fid: u32 = 0;
    let mut direction: u64 = 0;
    while running.load(Ordering::Relaxed) {
      let id = unsafe { get_next_input_id() };
      if let Some(mut buf) = depot.get_input_buf(id as usize) {
        unsafe { get_next_input(buf.as_mut_ptr(), &mut addr, &mut ctx, &mut order, &mut fid, &mut direction, buf.len()) };
        let new_path = executor.run_sync(&buf);
        let mut solcount = 1;
        if addr != 0 && branch_solcount.read().unwrap().contains_key(&(addr, ctx, order,direction)) {
          solcount = *branch_solcount.read().unwrap().get(&(addr,ctx, order,direction)).unwrap();
          solcount += 1;
          //info!("gencount is {}",count);
        }
        branch_solcount.write().unwrap().insert((addr,ctx,order,direction), solcount);
        if new_path.0 {
          info!("grading input derived from on input {} by flipping branch@ {:#01x} ctx {:#01x} order {}, it is a new input {}, saved as input #{}", fid, addr, ctx, order, new_path.0, new_path.1);
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




pub fn fuzz_loop(
    running: Arc<AtomicBool>,
    cmd_opt: CommandOpt,
    depot: Arc<Depot>,
    global_branches: Arc<GlobalBranches>,
    branch_gencount: Arc<RwLock<HashMap<(u64,u64,u32,u64),u32>>>,
    restart: bool,
    ) {

  let mut id: u32 = 0;

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

  let mut executor = Executor::new(
      cmd_opt,
      global_branches,
      depot.clone(),
      shmid,
      false, //not grading
      );

  let ptr = unsafe { libc::shmat(shmid, std::ptr::null(), 0) as *mut UnionTable};
  let table = unsafe { & *ptr };
  let branch_hitcount = Arc::new(RwLock::new(HashMap::<(u64,u64,u32,u64), u32>::new()));

  while running.load(Ordering::Relaxed) {
    if (id as usize) < depot.get_num_inputs() {
      //thread::sleep(time::Duration::from_millis(10));
      if let Some(buf) = depot.get_input_buf(id as usize) {
        let buf_cloned = buf.clone();
        //let path = depot.get_input_path(id).to_str().unwrap().to_owned();
        let gbranch_hitcount = branch_hitcount.clone();
        let gbranch_gencount = branch_gencount.clone();

        let (read_end, write_end) = pipe().unwrap();

        let handle = thread::Builder::new().stack_size(64 * 1024 * 1024).spawn(move || {
            dispatcher(table, gbranch_gencount, gbranch_hitcount, &buf_cloned, read_end);
            }).unwrap();


        let t_start = time::Instant::now();

        let mut child = executor.track(id as usize, &buf, read_end,write_end);
        close(write_end).map_err(|err| warn!("close write end {:?}", err)).ok();

        if handle.join().is_err() {
          error!("Error happened in listening thread!");
        }
        //dispatcher(table, gbranch_gencount, gbranch_hitcount, &buf_cloned, read_end);
        close(read_end).map_err(|err| warn!("close read end {:?}", err)).ok();
        
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
      thread::sleep(time::Duration::from_secs(1));
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
