use nix::unistd;
use nix::sys::stat;
//use std::io;
use std::io::prelude::*;
use std::io::BufReader;
use std::fs::File;

pub fn make_pipe() {
  match unistd::mkfifo("/tmp/wp", stat::Mode::S_IRWXU) {
    Ok(_) => println!("created"),
    Err(err) => println!("Error creating fifo: {}", err),
  }
}

pub fn read_pipe() -> Vec<(u32,u32)> {
  let f = File::open("/tmp/wp").expect("open pipe failed");
  let mut reader = BufReader::new(f);
  let mut ret = Vec::new();
  loop {
    let mut buffer = String::new();
    let num_bytes = reader.read_line(&mut buffer).expect("read pipe failed");
    //if not EOF
    if num_bytes !=0  {
      let tokens: Vec<&str> = buffer.trim().split(',').collect();
      let label = tokens[1].trim().parse::<u32>().expect("we expect u32 number in each line");
      let tid = tokens[0].trim().parse::<u32>().expect("we expect u32 number in each line");
      ret.push((tid,label)); 
    } else  {
      break;
    }
  }
  ret
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
    let v = read_pipe();
    println!("{:?}", v);
  }

}
