use nix::unistd;
use nix::sys::stat;

pub fn make_pipe() {
  match unistd::mkfifo("/tmp/wp", stat::Mode::S_IRWXU) {
    Ok(_) => println!("created"),
    Err(err) => println!("Error creating fifo: {}", err),
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  
  #[test]
  fn test_make_pipe() {
    make_pipe()
  }
}
