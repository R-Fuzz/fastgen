use super::*;
use fastgen_common::config;

use rand::{self, distributions::Uniform, Rng};

use std::{fmt, u8};
//use std::u16;
use std::{cmp, u32, u64};


#[derive(Clone)]
pub struct MutInput {
  pub value: Vec<u64>,
  disables: Vec<bool>,
}

impl MutInput {
    pub fn new() -> Self {
        Self {
            value: vec![],
            disables: vec![],
        }
    }
    

    pub fn val_len(&self) -> usize {
        self.value.len()
    }

    pub fn assign(&mut self, input: &Vec<(u32,u8)>) {
        for v in input {
          self.value.push(v.1 as u64);
          self.disables.push(false);
        }
    }

    pub fn update(&mut self, index: usize, direction: bool, delta: u64) {
        if !self.disables[index] {  
          if direction {
            self.value[index] = self.value[index].saturating_add(delta);
          } else {
            self.value[index] = self.value[index].saturating_sub(delta);
          }
        }
    }

    pub fn set_disable(&mut self, index: usize) {
        self.disables[index] = true;
    }

    pub fn reset_disable(&mut self) {
        for i in 0..self.val_len() {
          self.disables[i] = false;
        }
    }
    // the return value is unsigned!!
    pub fn get(&self, index: usize) -> u64 {
        self.value[index]
    }

    pub fn set(&mut self, index: usize, v: u64) {
        self.value[index] = v;
    }


    pub fn get_value(&self) -> Vec<u64> {
        self.value.clone()
    }

    pub fn bitflip(&mut self, i: usize) {
        let byte_i = i >> 3;
        let bit_i = i & 7;
        assert!(byte_i < self.val_len());
        self.value[byte_i] ^= 128 >> bit_i;
    }


    pub fn randomize_all(&mut self) {
        let mut rng = rand::thread_rng();
        self.randomize_all_with_weight(&mut rng, 3);
    }

    pub fn randomize_all_with_weight<T: Rng>(&mut self, rng: &mut T, weight: u32) {
        // 1/weight true
        let coin = rng.gen_bool(1.0 / weight as f64);
        if coin {
            self.randomize_all_uniform(rng);
        } else {
            self.randomize_all_mut_based(rng);
        }
    }

    pub fn randomize_all_uniform<T: Rng>(&mut self, rng: &mut T) {
        for i in 0..self.val_len() {
          self.value[i] = rng.gen();
        }
    }

    pub fn randomize_all_mut_based<T: Rng>(&mut self, rng: &mut T) {
        let byte_len = self.val_len() as u32;
        assert!(byte_len > 0);

        let use_stacking = if byte_len <= 4 {
            1 + rng.gen_range(0, 16)
        } else if byte_len <= 20 {
            1 + rng.gen_range(0, 64)
        } else {
            1 + rng.gen_range(0, 256)
        };

        // let choice_range = Range::new(0, 6);
        let choice_range = Uniform::new(0, 3);

        for _ in 0..use_stacking {
            match rng.sample(choice_range) {
                0 | 1 => {
                    // flip bit
                    let byte_idx: u32 = rng.gen_range(0, byte_len);
                    let bit_idx: u32 = rng.gen_range(0, 8);
                    self.value[byte_idx as usize] ^= 128 >> bit_idx;
                }
                2 => {
                    // random byte
                    let byte_idx: u32 = rng.gen_range(0, byte_len);
                    // self.randomize_one_byte(byte_idx as usize);
                    self.value[byte_idx as usize] = rng.gen();
                }
                _ => {}
            }
        }
    }
}

impl fmt::Debug for MutInput {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        for i in 0..self.val_len() {
            write!(f, "{}, ", self.get(i))?
        }
        Ok(())
    }
}
