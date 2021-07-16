//use super::{limit::SetLimit, *};
use super::{limit::SetLimit};
use fastgen_common::defs::*;
use byteorder::{LittleEndian, ReadBytesExt};
use libc;
use std::{
  collections::HashMap,
    fs,
    io::prelude::*,
    io::self,
    os::unix::{
      io::RawFd,
      net::{UnixListener, UnixStream},
      process::CommandExt,
    },
    path::Path,
    process::{Command, Stdio},
    time::Duration,
};
use nix::unistd::{close, pipe, read, write};

use crate::status_type::StatusType;

use nix::{
  sys::{
    select::{select, FdSet},
      signal::{kill, Signal},
      time::{TimeVal, TimeValLike},
  },
    unistd::Pid,
};


// Just meaningless value for forking a new child
static FORKSRV_NEW_CHILD: [u8; 4] = [8, 8, 8, 8];

const FORKSRV_FD: i32 = 198;

pub fn dup2(fd: i32, device: i32) -> Result<(), &'static str> {
  match unsafe { libc::dup2(fd, device) } {
    -1 => Err("dup2 failed"),
      _ => Ok(()),
  }
}

// Configure the target. setlimit, setsid, pipe_stdin, I borrowed the code from Angora fuzzer
pub trait ConfigTarget {
  fn setlimit(&mut self, memlimit: u64) -> &mut Self;
  fn setpipe(
      &mut self,
      st_read: RawFd,
      st_write: RawFd,
      ctl_read: RawFd,
      ctl_write: RawFd,
      ) -> &mut Self;
}


impl ConfigTarget for Command {
  fn setpipe(
      &mut self,
      st_read: RawFd,
      st_write: RawFd,
      ctl_read: RawFd,
      ctl_write: RawFd,
      ) -> &mut Self {
    let func = move || {
      match dup2(ctl_read, FORKSRV_FD) {
        Ok(_) => (),
        Err(_) => {
          return Err(io::Error::last_os_error());
        }
      }

      match dup2(st_write, FORKSRV_FD + 1) {
        Ok(_) => (),
        Err(_) => {
          return Err(io::Error::last_os_error());
        }
      }
      unsafe {
        libc::close(st_read);
        libc::close(st_write);
        libc::close(ctl_read);
        libc::close(ctl_write);
      }
      Ok(())
    };

    unsafe { self.pre_exec(func) }
  }


  fn setlimit(&mut self, memlimit: u64) -> &mut Self {
    if memlimit == 0 {
      return self;
    }
    let func = move || {
      let memlimit: libc::rlim_t = (memlimit as libc::rlim_t) << 20;
      let r = libc::rlimit {
rlim_cur: memlimit,
            rlim_max: memlimit,
      };
      let r0 = libc::rlimit {
rlim_cur: 0,
            rlim_max: 0,
      };

      let mut ret = unsafe { libc::setrlimit(libc::RLIMIT_AS, &r) };
      if ret < 0 {
        return Err(io::Error::last_os_error());
      }
      ret = unsafe { libc::setrlimit(libc::RLIMIT_CORE, &r0) };
      if ret < 0 {
        return Err(io::Error::last_os_error());
      }
      Ok(())
    };
    unsafe { self.pre_exec(func) }
  }
}

#[derive(Debug)]
pub struct Forksrv {
uses_asan: bool,
             is_stdin: bool,
             child_pid: Pid,
             ctl_write_end: RawFd,
             st_read_end: RawFd,
}

impl Forksrv {
  pub fn new(
      socket_path: &str,
      target: &(String, Vec<String>),
      envs: &HashMap<String, String>,
      fd: RawFd,
      is_stdin: bool,
      uses_asan: bool,
      time_limit: u64,
      mem_limit: u64,
      ) -> Result<Self,&'static str> {
    debug!("socket_path: {:?}", socket_path);

    // status pipe and ctrl pipe
    let (ctl_read_end, ctl_write_end) = pipe().unwrap();
    let (st_read_end, st_write_end) = pipe().unwrap();


    let mut envs_fk = envs.clone();
    envs_fk.insert(ENABLE_FORKSRV.to_string(), String::from("TRUE"));
    envs_fk.insert(FORKSRV_SOCKET_PATH_VAR.to_string(), socket_path.to_owned());

    match Command::new(&target.0)
      .args(&target.1)
      .stdin(Stdio::null())
      .envs(&envs_fk)
      .stdout(Stdio::null())
      .stderr(Stdio::null())
      .setlimit(mem_limit) 
      .mem_limit(mem_limit.clone())
      .setsid()
      .pipe_stdin(fd, is_stdin)
      .setpipe(st_read_end,st_write_end,ctl_read_end,ctl_write_end)
      .spawn() 
      {
        Ok(_) => {}
        Err(err) => {
          warn!(
              "Could not spawn the forkserver: {:#?}",
              err
              );
          return Err("can't spawn");
        }
      }

    close(ctl_read_end);
    close(st_write_end);

    debug!("All right -- Init ForkServer {} successfully!", socket_path);

    Ok(Forksrv {
        uses_asan,
        is_stdin,
        child_pid: Pid::from_raw(0),
        ctl_write_end,
        st_read_end,
        })
  }

  pub fn read_st(&mut self) -> Result<(usize, i32), &'static str> {
    let mut buf: [u8; 4] = [0u8; 4];
    if let Ok(rlen) = read(self.st_read_end, &mut buf) {
      let val: i32 = i32::from_ne_bytes(buf);
      Ok((rlen, val))
    } else {
      Err("read error")
    }
  }

  pub fn write_ctl(&mut self, buf: &[u8; 4]) -> Result<usize, &'static str> {
    //if let Ok(slen) = self.ctl_pipe.write(&val.to_ne_bytes()) {
    if let Ok(slen) = write(self.ctl_write_end, buf) {
      Ok(slen)
    } else {
      Err("write ctl error")
    }
  }

  pub fn read_st_timed(&mut self, timeout: &mut TimeVal) -> Result<Option<i32>, &'static str> {
    let mut buf: [u8; 4] = [0u8; 4];
    let mut readfds = FdSet::new();
    let mut copy = *timeout;
    readfds.insert(self.st_read_end);
    if let Ok(sret) = select(
        Some(readfds.highest().unwrap() + 1),
        &mut readfds,
        None,
        None,
        &mut copy,
        ) {
      if sret > 0 {
        if let Ok(len) = read(self.st_read_end, &mut buf) {
          if len < 4 {
            return Err("not enough bytes read");
          }
          let val: i32 = i32::from_ne_bytes(buf);
          Ok(Some(val))
        } else {
          return Err("read error");
        }
      } else {
        Ok(None)
      }
    } else {
      Err("select error")
    }
  }

  pub fn child_pid(&self) -> Pid {
    self.child_pid
  }

  pub fn set_child_pid(&mut self, child_pid: Pid) {
    self.child_pid = child_pid;
  }

  pub fn run(&mut self) -> StatusType {

    if let Ok(send_len) = self.write_ctl(&FORKSRV_NEW_CHILD) {
      if send_len != 4 {
        warn!("Unable to request new process from fork server (OOM?)");
        return StatusType::Error;
      }
    } else {
      warn!("Fail to write pipe!");
      return StatusType::Error;
    }

    if let Ok((recv_pid_len, pid)) = self.read_st() {
      if recv_pid_len != 4 {
        warn!("Unable to request new process from fork server (OOM?)");
        return StatusType::Error;
      }

      if pid <= 0 {
        warn!("Unable to request new process from fork server (OOM?)");
        return StatusType::Error;
      }

      self.set_child_pid(Pid::from_raw(pid));
    } else {
      warn!("Fail to read pipe!");
      return StatusType::Error;
    }

    let mut timeout = TimeVal::seconds(1);

    if let Ok(Some(status)) = self.read_st_timed(&mut timeout) {
      let signaled = libc::WIFSIGNALED(status);
      let exit_code = libc::WEXITSTATUS(status);
      if signaled || (self.uses_asan && exit_code ==MSAN_ERROR_CODE) {
        StatusType::Crash
      } else {
        StatusType::Normal
      }
    } else {
      let _ = kill(self.child_pid(), Signal::SIGKILL); 
      if let Ok((recv_status_len, _)) = self.read_st() {
        if recv_status_len != 4 {
          warn!("Could not kill timed-out child");
          StatusType::Error;
        }
      } else {
        StatusType::Error;
      }
      return StatusType::Timeout;
    }
  }
}

