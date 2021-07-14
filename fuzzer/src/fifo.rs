use nix::unistd;
use nix::sys::stat;
//use std::io;
use std::io::BufReader;
use std::collections::VecDeque;
use std::os::unix::io::{FromRawFd, RawFd};
use byteorder::{LittleEndian, ReadBytesExt};
use std::{
    fs::File,
    io::{self, Read},
};


struct PipeMsg {
  msgtype: u32, //gep, cond, add_constraints, strcmp 
  tid: u32, 
  label: u32,
  result: u64, //direction for conditional branch, index for GEP
  addr: u64,
  ctx: u64,
  localcnt: u32,
}

impl PipeMsg {
  fn from_reader(mut rdr: impl Read) -> io::Result<Self> {
    let msgtype = rdr.read_u32::<LittleEndian>()?;
    let tid = rdr.read_u32::<LittleEndian>()?;
    let label = rdr.read_u32::<LittleEndian>()?;
    let result = rdr.read_u64::<LittleEndian>()?;
    let addr = rdr.read_u64::<LittleEndian>()?;
    let ctx = rdr.read_u64::<LittleEndian>()?;
    let localcnt = rdr.read_u32::<LittleEndian>()?;

    Ok(PipeMsg{
        msgtype,
        tid,
        label,
        result,
        addr,
        ctx,
        localcnt,
        })
  }
}

pub fn make_pipe() {
  match unistd::mkfifo("/tmp/wp", stat::Mode::S_IRWXU) {
    Ok(_) => println!("created"),
      Err(err) => println!("Error creating fifo: {}", err),
  }
}

pub fn read_pipe(piped: RawFd) -> (Vec<(u32,u32,u64,u64,u64,u32,u32)>, VecDeque<Vec<u8>>) {
  let f = unsafe { File::from_raw_fd(piped) };
  let mut reader = BufReader::new(f);
  let mut ret = Vec::new();
  let mut retdata = VecDeque::new();
  loop {
    let msg = PipeMsg::from_reader(&mut reader);
    if let Ok(rawmsg) = msg {
      let tid = rawmsg.tid; 
      let label = rawmsg.label;
      let direction = rawmsg.result;
      let addr = rawmsg.addr;
      let ctx = rawmsg.ctx;
      let isgep  = rawmsg.msgtype;
      let order = rawmsg.localcnt;
      ret.push((tid,label,direction,addr,ctx,order,isgep));
      if isgep == 2 {
        let mut data = Vec::new();
        for _i in 0..direction as usize {
            if let Ok(cur) = reader.read_u8() {
              data.push(cur);
            } else {
              break;
            }
        } 
        if data.len() < direction as usize {
          break;
        }
        retdata.push_back(data);
      }
    } else  {
      break;
    }
  }
  (ret,retdata)
}

#[cfg(test)]
mod tests {
  use super::*;

#[test]
  fn test_make_pipe() {
    make_pipe()
  }

#[test]
  fn test_read_pipe() {
    let (v,w) = read_pipe(2);
    println!("{:?}", v);
  }

}
