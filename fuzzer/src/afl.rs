// Modify input randomly like AFL.
// All the byte offsets in the input is the input.
// Random pick offsets, then flip, add/sub ..
// And GE algorithm.

use rand::{self, distributions::Uniform, Rng, RngCore};
use fastgen_common::{config};
use crate::executor::Executor;
use crate::interesting_val::*;

static IDX_TO_SIZE: [usize; 4] = [1, 2, 4, 8];

pub fn set_val_in_buf(buf: &mut Vec<u8>, off: usize, size: usize, val: u64) {
    match size {
        1 => {
            let v = &mut buf[off];
            *v = val as u8;
        },
        2 => {
            let v = unsafe { &mut *(&mut buf[off] as *mut u8 as *mut u16) };
            *v = val as u16;
        },
        4 => {
            let v = unsafe { &mut *(&mut buf[off] as *mut u8 as *mut u32) };
            *v = val as u32;
        },
        8 => {
            let v = unsafe { &mut *(&mut buf[off] as *mut u8 as *mut u64) };
            *v = val as u64;
        },
        _ => {
            panic!("strange arg off and size: {}, {}", off, size);
        },
    };
}

pub fn update_val_in_buf(
    buf: &mut Vec<u8>,
    sign: bool,
    off: usize,
    size: usize,
    direction: bool,
    delta: u64,
) {
    match size {
        1 => {
            if sign {
                let v = buf[off] as i8;
                buf[off] = if direction {
                    v.wrapping_add(delta as i8) as u8
                } else {
                    v.wrapping_sub(delta as i8) as u8
                };
            } else {
                let v = &mut buf[off];
                if direction {
                    *v = v.wrapping_add(delta as u8);
                } else {
                    *v = v.wrapping_sub(delta as u8);
                }
            }
        },
        2 => {
            if sign {
                let v = unsafe { &mut *(&mut buf[off] as *mut u8 as *mut i16) };
                if direction {
                    *v = v.wrapping_add(delta as i16);
                } else {
                    *v = v.wrapping_sub(delta as i16);
                }
            } else {
                let v = unsafe { &mut *(&mut buf[off] as *mut u8 as *mut u16) };
                if direction {
                    *v = v.wrapping_add(delta as u16);
                } else {
                    *v = v.wrapping_sub(delta as u16);
                }
            }
        },
      4 => {
            if sign {
                let v = unsafe { &mut *(&mut buf[off] as *mut u8 as *mut i32) };
                if direction {
                    *v = v.wrapping_add(delta as i32);
                } else {
                    *v = v.wrapping_sub(delta as i32);
                }
            } else {
                let v = unsafe { &mut *(&mut buf[off] as *mut u8 as *mut u32) };
                if direction {
                    *v = v.wrapping_add(delta as u32);
                } else {
                    *v = v.wrapping_sub(delta as u32);
                }
            }
        },
        8 => {
            if sign {
                let v = unsafe { &mut *(&mut buf[off] as *mut u8 as *mut i64) };
                if direction {
                    *v = v.wrapping_add(delta as i64);
                } else {
                    *v = v.wrapping_sub(delta as i64);
                }
            } else {
                let v = unsafe { &mut *(&mut buf[off] as *mut u8 as *mut u64) };
                if direction {
                    *v = v.wrapping_add(delta as u64);
                } else {
                    *v = v.wrapping_sub(delta as u64);
                }
            }
        },
        _ => {
            panic!("strange arg off and size: {}, {}", off, size);
        },
    };
}


pub fn run_afl_mutator(executor: &mut Executor, buf: &mut Vec<u8>) {
  info!("run afl_mutator");
  //TODO if first ime
  afl_len(executor, buf);

  let mut max_times: usize = config::MAX_SPLICE_TIMES * 5;
  for _i in 0..max_times {
    if !splice(executor, buf) {
      break;
    }
  }

  let max_stacking = if buf.len() <= 16 {
    64
  } else {
    256
  };
/*
  let max_choice = if config::ENABLE_MICRO_RANDOM_LEN {
    8
  } else {
    6
  };
*/
  let max_choice = 8;

  let choice_range = Uniform::new(0, max_choice);

  max_times += config::MAX_HAVOC_FLIP_TIMES * 5;

  for _i in 0..max_times {
    let mut buf = buf.clone();
    havoc_flip(&mut buf, max_stacking, choice_range);
    executor.run_sync(&buf);
  }
}

fn locate_diffs(buf1: &Vec<u8>, buf2: &Vec<u8>, len: usize) -> (Option<usize>, Option<usize>) {
  let mut first_loc = None;
  let mut last_loc = None;

  for i in 0..len {
    if buf1[i] != buf2[i] {
      if first_loc.is_none() {
        first_loc = Some(i);
      }
      last_loc = Some(i);
    }
  }

  (first_loc, last_loc)
}

fn splice_two_vec(buf1: &Vec<u8>, buf2: &Vec<u8>) -> Option<Vec<u8>> {
  let len = std::cmp::min(buf1.len(), buf2.len());
  if len < 2 {
    return None;
  }
  let (f_loc, l_loc) = locate_diffs(buf1, buf2, len);
  if f_loc.is_none() || l_loc.is_none() {
    return None;
  }
  let f_loc = f_loc.unwrap();
  let l_loc = l_loc.unwrap();
  if f_loc == l_loc {
    return None;
  }

  let split_at = f_loc + rand::random::<usize>() % (l_loc - f_loc);
  Some([&buf1[..split_at], &buf2[split_at..]].concat())
}

// GE algorithm
fn splice(executor: &mut Executor, buf: &mut Vec<u8>) -> bool {
  let buf1 = buf.clone();
  let buf2 = executor.random_input_buf();
  if let Some(new_buf) = splice_two_vec(&buf1, &buf2) {
    executor.run_sync(&new_buf);
    true
  } else {
    false
  }
}

// TODO both endian?
fn havoc_flip(buf: &mut Vec<u8>, max_stacking: usize, choice_range: Uniform<u32>) {
  let mut rng = rand::thread_rng();
  let mut byte_len = buf.len() as u32;
  let use_stacking = 1 + rng.gen_range(0, max_stacking);

  for _ in 0..use_stacking {
    match rng.sample(choice_range) {
      0 | 1 => {
        // flip bit
        let byte_idx: u32 = rng.gen_range(0, byte_len);
        let bit_idx: u32 = rng.gen_range(0, 8);
        buf[byte_idx as usize] ^= 128 >> bit_idx;
      },

        2 | 3 => {
          //add or sub
          let n: u32 = rng.gen_range(0, 3);
          let size = IDX_TO_SIZE[n as usize];
          if byte_len > size as u32 {
            let byte_idx: u32 = rng.gen_range(0, byte_len - size as u32);
            let v: u32 = rng.gen_range(0, config::MUTATE_ARITH_MAX);
            let direction: bool = rng.gen();
            update_val_in_buf(
                buf,
                false,
                byte_idx as usize,
                size,
                direction,
                v as u64,
                );
          }
        },
        4 => {
          // set interesting value
          let n: u32 = rng.gen_range(0, 3);
          let size = IDX_TO_SIZE[n as usize];
          if byte_len > size as u32 {
            let byte_idx: u32 = rng.gen_range(0, byte_len - size as u32);
            let vals = get_interesting_bytes(size);
            let wh = rng.gen_range(0, vals.len() as u32);
            set_val_in_buf(buf, byte_idx as usize, size, vals[wh as usize]);
          }
        },

        5 => {
          // random byte
          let byte_idx: u32 = rng.gen_range(0, byte_len);
          let val: u8 = rng.gen();
          buf[byte_idx as usize] = val;
        },
        6 => {
          // delete bytes
          let remove_len: u32 = rng.gen_range(1, 5);
          if byte_len > remove_len {
            byte_len -= remove_len;
            //assert!(byte_len > 0);
            let byte_idx: u32 = rng.gen_range(0, byte_len);
            for _ in 0..remove_len {
              buf.remove(byte_idx as usize);
            }
          }
        },
        7 => {
          // insert bytes
          let add_len = rng.gen_range(1, 5);
          let new_len = byte_len + add_len;
          if new_len < config::MAX_INPUT_LEN as u32 {
            let byte_idx: u32 = rng.gen_range(0, byte_len);
            byte_len = new_len;
            for i in 0..add_len {
              buf.insert((byte_idx + i) as usize, rng.gen());
            }
          }
        },
        _ => {},
    }
  }
}

fn random_len(executor: &mut Executor, buf: &mut Vec<u8>) {
  let len = buf.len();
  if len > config::MAX_INPUT_LEN {
    return;
  }

  // let step = std::cmp::max( len / config::INFLATE_MAX_ITER_NUM + 1, 5);
  let orig_len = buf.len();
  let mut rng = rand::thread_rng();

  let mut buf = buf.clone();
  for _ in 0..config::RANDOM_LEN_NUM {
    let step = rng.gen::<usize>() % orig_len + 1;
    let mut v = vec![0u8; step];
    rng.fill_bytes(&mut v);
    buf.append(&mut v);
    if buf.len() < config::MAX_INPUT_LEN {
      executor.run_sync(&buf);
    } else {
      break;
    }
  }
}

fn add_small_len(executor: &mut Executor, buf: &mut Vec<u8>) {
  let len = buf.len();
  if len > config::MAX_INPUT_LEN {
    return;
  }

  let mut rng = rand::thread_rng();
  let mut buf = buf.clone();
  let mut step = 1;
  for _ in 0..4 {
    let mut v = vec![0u8; step];
    rng.fill_bytes(&mut v);
    buf.append(&mut v);
    step = step * 2;
    if buf.len() < config::MAX_INPUT_LEN {
      executor.run_sync(&buf);
    } else {
      break;
    }
  }
}

fn afl_len(executor: &mut Executor, buf: &mut Vec<u8>) {
  if config::ENABLE_RANDOM_LEN {
    random_len(executor, buf);
  } else {
    add_small_len(executor,buf);
  }
}
