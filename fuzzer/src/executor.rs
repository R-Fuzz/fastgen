use crate::status_type::StatusType;
use super::{limit::SetLimit, *};

use crate::pipe_fd::PipeFd;
use crate::forksrv::Forksrv;

use crate::{
  branches, command,shm_conds,
    depot, 
};
use fastgen_common::{config, defs};

use std::{
  collections::HashMap,
    process::{Command, Stdio},
    sync::{
      atomic::{compiler_fence, Ordering},
      Arc, Mutex,
    },
    time,
};
use wait_timeout::ChildExt;
use std::os::unix::io::RawFd;
use std::os::unix::process::CommandExt;
use std::io::{self};
use nix::unistd::{close, pipe, read, write};

pub fn dup2(fd: i32, device: i32) -> Result<(), &'static str> {
  match unsafe { libc::dup2(fd, device) } {
    -1 => Err("dup2 failed"),
      _ => Ok(()),
  }
}

pub trait ConfigTrack {
  fn setpipe(
      &mut self,
      track_read: RawFd,
      track_write: RawFd,
      ) -> &mut Self;
}


impl ConfigTrack for Command {
  fn setpipe(
      &mut self,
      track_read: RawFd,
      track_write: RawFd,
      ) -> &mut Self {
    let func = move || {

      match dup2(track_write, 200) {
        Ok(_) => (),
        Err(_) => {
          return Err(io::Error::last_os_error());
        }
      }
      unsafe {
        libc::close(track_read);
        libc::close(track_write);
      }
      Ok(())
    };

    unsafe { self.pre_exec(func) }
  }
}


pub struct Executor {
  pub cmd: command::CommandOpt,
      pub branches: branches::Branches,
      pub t_conds: shm_conds::ShmConds,
      envs: HashMap<String, String>,
      //forksrv: Result<Forksrv,&'static str>,
      forksrv: Option<Forksrv>,
      depot: Arc<depot::Depot>,
      fd: PipeFd,
      tmout_cnt: usize,
      pub has_new_path: bool,
      pub shmid: i32,
      pub fl: Arc<Mutex<u32>>,
}

impl Executor {
  pub fn new(
      cmd: command::CommandOpt,
      global_branches: Arc<branches::GlobalBranches>,
      depot: Arc<depot::Depot>,
      shmid: i32,
      is_grading: bool,
      forklock: Arc<Mutex<u32>>,
      ) -> Self {
    // ** Share Memory **
    let branches = branches::Branches::new(global_branches);
    let t_conds = shm_conds::ShmConds::new();

    // ** Envs **
    let mut envs = HashMap::new();
    envs.insert(
        defs::ASAN_OPTIONS_VAR.to_string(),
        defs::ASAN_OPTIONS_CONTENT.to_string(),
        );
    envs.insert(
        defs::MSAN_OPTIONS_VAR.to_string(),
        defs::MSAN_OPTIONS_CONTENT.to_string(),
        );
    envs.insert(
        defs::BRANCHES_SHM_ENV_VAR.to_string(),
        branches.get_id().to_string(),
        );
    envs.insert(
        defs::COND_STMT_ENV_VAR.to_string(),
        t_conds.get_id().to_string(),
        );
    envs.insert(
        defs::LD_LIBRARY_PATH_VAR.to_string(),
        cmd.ld_library.clone(),
        );

    let fd = pipe_fd::PipeFd::new(&cmd.out_file);
    let forksrv = if is_grading { forksrv::Forksrv::new(
          &cmd.forksrv_socket_path,
          &cmd.main,
          &envs,
          fd.as_raw_fd(),
          cmd.is_stdin,
          cmd.uses_asan,
          cmd.time_limit,
          cmd.mem_limit,
          forklock.clone(),
          ) } else {
            None
          };

    Self {
      cmd,
        branches,
        t_conds,
        envs,
        forksrv,
        depot,
        fd,
        tmout_cnt: 0,
        has_new_path: false,
        shmid,
        fl: forklock.clone(),
    }
  }

  pub fn rebind_forksrv(&mut self) {

    {
      // delete the old forksrv
      self.forksrv = None;
    }
    let fs = forksrv::Forksrv::new(
        &self.cmd.forksrv_socket_path,
        &self.cmd.main,
        &self.envs,
        self.fd.as_raw_fd(),
        self.cmd.is_stdin,
        self.cmd.uses_asan,
        self.cmd.time_limit,
        self.cmd.mem_limit,
        self.fl.clone(),
        );
    self.forksrv = fs;

  }

  pub fn track(&mut self, id: usize, buf: &Vec<u8>) -> (std::process::Child, RawFd) {
    //FIXME
    let e = format!("taint_file={} tid={} shmid={} pipeid=200", &self.cmd.out_file, &id, &self.shmid);
    debug!("Track {}, e is {}", &id, e);
    self.envs.insert(
        defs::TAINT_OPTIONS.to_string(),
        e,
        );


    self.write_test(buf);

    compiler_fence(Ordering::SeqCst);
    let (child, read_end) = self.run_track(
        &self.cmd.track,
        config::MEM_LIMIT_TRACK,
        //self.cmd.time_limit *
        config::TIME_LIMIT_TRACK,
        );
    compiler_fence(Ordering::SeqCst);
    (child,read_end)
/*
    if ret_status != StatusType::Normal {
      error!(
          "Crash or hang while tracking! -- {:?},  id: {}",
          ret_status, id
          );
      return;
    }

*/
  }


  fn do_if_has_new(&mut self, buf: &Vec<u8>, status: StatusType) -> (bool,usize) {
    // new edge: one byte in bitmap
    let has_new_path = self.branches.has_new(status);
    let mut new_id = 0;

    if has_new_path {
      self.has_new_path = true;
      new_id = self.depot.save(status, &buf) - 1;
    }
    (has_new_path,new_id)
  }

  pub fn run(&mut self, buf: &Vec<u8>) -> StatusType {
    self.run_init();
    let status = self.run_inner(buf);
    self.do_if_has_new(buf, status);
    self.check_timeout(status)
  }

  pub fn run_sync(&mut self, buf: &Vec<u8>) -> (bool,usize)  {
    self.run_init();
    let status = self.run_inner(buf);
    let ret = self.do_if_has_new(buf, status);
    self.check_timeout(status);
    ret
  }

  pub fn run_sync_with_cond(&mut self, buf: &Vec<u8>, cmpid: u32, ctx: u32, order: u32) -> (bool,usize)  {
    self.run_init();
    self.t_conds.set(cmpid,ctx,order);
    let status = self.run_inner(buf);
    let ret = self.do_if_has_new(buf, status);
    self.check_timeout(status);
    ret
  }

  pub fn get_cond(&mut self) -> u64 {
    return self.t_conds.cond.condition;
  }


  pub fn run_norun(&mut self, buf: &Vec<u8>)  {
    let status = StatusType::Normal;
    self.depot.save(status, &buf);
  }


  fn run_init(&mut self) {
    self.has_new_path = false;
  }

  fn check_timeout(&mut self, status: StatusType) -> StatusType {
    let mut ret_status = status;
    if ret_status == StatusType::Error {
      warn!("timeout we are rebinding forksrv");
      self.rebind_forksrv();
      ret_status = StatusType::Timeout;
    }

    if ret_status == StatusType::Timeout {
      self.tmout_cnt = self.tmout_cnt + 1;
      if self.tmout_cnt >= config::TMOUT_SKIP {
        ret_status = StatusType::Skip;
        self.tmout_cnt = 0;
      }
    } else {
      self.tmout_cnt = 0;
    };

    ret_status
  }

  fn run_inner(&mut self, buf: &Vec<u8>) -> StatusType {
    self.write_test(buf);

    self.branches.clear_trace();

    compiler_fence(Ordering::SeqCst);
    let mut ret_status = StatusType::Error;
    if let Some(ref mut fs) = self.forksrv {
      ret_status = fs.run()
    } else {
      warn!("run does not go through forksrv and we rebinding");
      //ret_status = self.run_target(&self.cmd.main, self.cmd.mem_limit, self.cmd.time_limit);
      self.rebind_forksrv();
      if let Some(ref mut fs) = self.forksrv {
        ret_status = fs.run()
      }
    };
    compiler_fence(Ordering::SeqCst);

    ret_status
  }


  pub fn random_input_buf(&self) -> Option<Vec<u8>> {
    let id = self.depot.next_random();
    self.depot.get_input_buf(id)
  }

  fn write_test(&mut self, buf: &Vec<u8>) {
    self.fd.write_buf(buf);
    if self.cmd.is_stdin {
      self.fd.rewind();
    }
  }

  fn run_target(
      &self,
      target: &(String, Vec<String>),
      mem_limit: u64,
      time_limit: u64,
      ) -> StatusType {
    let mut cmd = Command::new(&target.0);
    let mut child = cmd
      .args(&target.1)
      //  .stdin(Stdio::null())
      .env_clear()
      .envs(&self.envs)
      .stdout(Stdio::null())
      .stderr(Stdio::null())
      .mem_limit(mem_limit.clone())
      .setsid()
      .pipe_stdin(self.fd.as_raw_fd(), self.cmd.is_stdin)
      .spawn()
      .expect("Could not run target");


    let timeout = time::Duration::from_secs(time_limit);
    let ret = match child.wait_timeout(timeout) {
    //let ret = match child.try_wait() {
      Ok(Some(status)) => {
        if let Some(status_code) = status.code() {
          if self.cmd.uses_asan && status_code == defs::MSAN_ERROR_CODE
          {
            StatusType::Crash
          } else {
            StatusType::Normal
          }
        } else {
          StatusType::Crash
        }
      }
      Ok(None) => {
        // Timeout
        // child hasn't exited yet
        child.kill().map_err(|err| warn!("child kill {:?}", err)).ok();
        child.wait().map_err(|err| warn!("child wait {:?}", err)).ok();
        StatusType::Timeout
      }
      Err(_) => { StatusType::Timeout }
    };
    ret
  }

  fn run_track(
      &self,
      target: &(String, Vec<String>),
      mem_limit: u64,
      time_limit: u64,
      ) -> (std::process::Child, RawFd) {
    let guard = self.fl.lock().unwrap();
    let (read_end, write_end) = pipe().unwrap();
    let mut cmd = Command::new("timeout");
    let mut child = cmd
      .arg("102")
      .arg(&target.0)
      .args(&target.1)
      //  .stdin(Stdio::null())
      .env_clear()
      .envs(&self.envs)
      .stdout(Stdio::null())
      .stderr(Stdio::null())
      .mem_limit(mem_limit.clone())
      .setsid()
      .setpipe(read_end, write_end)
      .pipe_stdin(self.fd.as_raw_fd(), self.cmd.is_stdin)
      .spawn()
      .expect("Could not run target");

    close(write_end);
    (child, read_end)
/*
    let timeout = time::Duration::from_secs(time_limit);
    let ret = match child.wait_timeout(timeout) {
      Ok(Some(status)) => {
        if let Some(status_code) = status.code() {
          if self.cmd.uses_asan && status_code == defs::MSAN_ERROR_CODE
          {
            StatusType::Crash
          } else {
            StatusType::Normal
          }
        } else {
          StatusType::Crash
        }
      }
      Ok(None) => {
        // Timeout
        // child hasn't exited yet
        child.kill().map_err(|err| warn!("child kill {:?}", err)).ok();
        child.wait().map_err(|err| warn!("child wait {:?}", err)).ok();
        StatusType::Timeout
      }
      Err(_) => { StatusType::Timeout }
    };
    ret
*/
  }

}
