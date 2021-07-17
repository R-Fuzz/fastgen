use fastgen_common::{config, defs};
use std::env;

use byteorder::{LittleEndian, WriteBytesExt};
use libc;
use std::{io::prelude::*, os::unix::net::UnixStream, process, time::Duration};
//use nix::unistd::{read, write};

pub fn start_forkcli() {
  let mut sig_buf = [0u8; 4];
  unsafe { super::context::reset_context(); }
  loop {
    //read from ctrl pipe
    if unsafe { libc::read(198, sig_buf.as_mut_ptr() as *mut libc::c_void, 4) }  != 4 {
      //if socket.read(&mut sig_buf).is_err() {
      process::exit(0);
    }

    let child_pid = unsafe { libc::fork() };

    if child_pid == 0 {
      unsafe { super::context::reset_context(); }
      return;
    }

    let mut pid_buf = vec![];
    pid_buf
      .write_i32::<LittleEndian>(child_pid)
      .expect("Could not write to child.");
    //write to status pipe
    //if write(199, &pid_buf).is_err() {
    if unsafe { libc::write(199, pid_buf.as_ptr() as *const libc::c_void, 4) }  != 4 {
      process::exit(1);
    }

    let mut status: libc::c_int = 0;
    if unsafe { libc::waitpid(child_pid, &mut status as *mut libc::c_int, 0) } < 0 {
      process::exit(1);
    }

    let mut status_buf = vec![];
    status_buf
      .write_i32::<LittleEndian>(status)
      .expect("Could not write to child.");
    //if write(199, &status_buf).is_err() {
    if unsafe { libc::write(199, status_buf.as_ptr() as *const libc::c_void, 4) }  != 4 {
      process::exit(1);
    }
  }
}

