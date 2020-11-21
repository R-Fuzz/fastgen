use crate::{
    branches::GlobalBranches, command::CommandOpt, depot::Depot,
    executor::Executor,
};
use rand::prelude::*;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, RwLock,
};

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
}
