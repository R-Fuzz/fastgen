use fastgen_common::defs;
use chrono::prelude::Local;
use std::{
    fs,
    path::{PathBuf, Path},
    sync::{
      atomic::{AtomicBool, Ordering},
      Arc, RwLock,
    },
    thread,
};

use std::time;
use crate::{branches, check_dep, command, depot, sync, executor};
use ctrlc;
use pretty_env_logger;
use crate::fuzz_loop;
use crate::cpp_interface::*;
use fastgen_common::config;
use std::collections::HashMap;

pub fn fuzz_main(
    in_dir: &str,
    out_dir: &str,
    track_target: &str,
    pargs: Vec<String>,
    //TODO jobs
    _num_jobs: usize,
    _num_graders: usize,
    mem_limit: u64,
    time_limit: u64,
    sync_afl: bool,
    ) {
  pretty_env_logger::init();

  let (seeds_dir, angora_out_dir) = initialize_directories(in_dir, out_dir, sync_afl);

  let restart = in_dir == "-";
  let command_option = command::CommandOpt::new(
      track_target,
      pargs,
      &angora_out_dir,
      mem_limit,
      time_limit,
      );
  info!("{:?}", command_option);

  check_dep::check_dep(in_dir, out_dir, &command_option);

  let depot = Arc::new(depot::Depot::new(seeds_dir, &angora_out_dir));
  info!("{:?}", depot.dirs);

  let global_branches = Arc::new(branches::GlobalBranches::new());
  let branch_gencount = Arc::new(RwLock::new(HashMap::<(u64,u64,u32,u64), u32>::new()));
  let branch_solcount = Arc::new(RwLock::new(HashMap::<(u64,u64,u32,u64), u32>::new()));
  let running = Arc::new(AtomicBool::new(true));
  set_sigint_handler(running.clone());

  let mut executor = executor::Executor::new(
      command_option.specify(0),
      global_branches.clone(),
      depot.clone(),
      0,
      );

  sync::sync_depot(&mut executor, running.clone(), &depot.dirs.seeds_dir);

  if depot.empty() {
    error!("Please ensure that seed directory - {:?} has ang file", depot.dirs.seeds_dir);
  }

  unsafe { init_core(config::SAVING_WHOLE, config::USE_CODECACHE); }
  let mut handlers = vec![];
  for g in 0.._num_graders
  {
    let r = running.clone();
    let d = depot.clone();
    let b = global_branches.clone();
    let cmd = command_option.specify(3+g);
    let bg = branch_gencount.clone();
    let bs = branch_solcount.clone();
    let handle = thread::spawn(move || {
        //fuzz_loop::branch_checking(r, cmd, d, b, bg, bs);
        fuzz_loop::grading_loop(r, cmd, d, b, bg, bs);
        });
    handlers.push(handle);
  }
  { 

    let r = running.clone();
    let d = depot.clone();
    let b = global_branches.clone();
    let cmd = command_option.specify(2);
    let bg = branch_gencount.clone();
    let handle = thread::Builder::new().stack_size(64 * 1024 * 1024).spawn(move || {
        fuzz_loop::fuzz_loop(r, cmd, d, b, bg, restart);
        }).unwrap();
    handlers.push(handle);
  }
   

  main_thread_sync(
    out_dir,
    sync_afl,
    running.clone(),
    &mut executor,
  );

  
  for handle in handlers {
    if handle.join().is_err() {
        error!("Error happened in fuzzing thread!");
    }
  }
}

fn initialize_directories(in_dir: &str, out_dir: &str, sync_afl: bool) -> (PathBuf, PathBuf) {
  let angora_out_dir = if sync_afl {
    gen_path_afl(out_dir)
  } else {
    PathBuf::from(out_dir)
  };


  let restart = in_dir == "-";
  if !restart {
    fs::create_dir(&angora_out_dir).expect("Output directory has existed!");
  }

  

  let workdir = PathBuf::from("angora");

  let out_dir = &angora_out_dir;
  let seeds_dir = if restart {
    let orig_out_dir = workdir.with_extension(Local::now().to_rfc3339());
    println!("orig out dir is {:?}",orig_out_dir);
    fs::rename(&out_dir, orig_out_dir.clone()).unwrap();
    fs::create_dir(&out_dir).unwrap();
    PathBuf::from(orig_out_dir).join(defs::INPUTS_DIR)
  } else {
    PathBuf::from(in_dir)
  };

  (seeds_dir, angora_out_dir)
}

fn gen_path_afl(out_dir: &str) -> PathBuf {
  let base_path = PathBuf::from(out_dir);
  let create_dir_result = fs::create_dir(&base_path);
  if create_dir_result.is_err() {
    warn!("dir has existed. {:?}", base_path);
  }
  base_path.join(defs::ANGORA_DIR_NAME)
}

fn set_sigint_handler(r: Arc<AtomicBool>) {
  ctrlc::set_handler(move || {
      warn!("Ending Fuzzing.");
      r.store(false, Ordering::SeqCst);
      })
  .expect("Error setting SIGINT handler!");
}



fn main_thread_sync(
  out_dir: &str,
  sync_afl: bool,
  running: Arc<AtomicBool>,
  executor: &mut executor::Executor,
) {
  let sync_dir = Path::new(out_dir);
  let mut synced_ids = HashMap::new();
  if sync_afl {
    sync::sync_afl(executor, running.clone(), sync_dir, &mut synced_ids);
  }
  let mut sync_counter = 1;
  while running.load(Ordering::SeqCst) {
    thread::sleep(time::Duration::from_secs(5));
    sync_counter -= 1;
    if sync_afl && sync_counter <= 0 {
      sync::sync_afl(executor, running.clone(), sync_dir, &mut synced_ids);
      sync_counter = 12;
    }
  }
}
