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
        executor.track(id, &buf);
        id = id + 1;
      }
      trace!("runninng one epoch and number is {}", depot.get_num_inputs());
      thread::sleep(time::Duration::from_secs(1));
    }
}
