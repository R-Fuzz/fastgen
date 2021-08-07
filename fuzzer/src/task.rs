use inkwell::context::Context;
use inkwell::execution_engine::JitFunction;
use inkwell::OptimizationLevel;
use std::collections::HashMap;
use crate::jit::JigsawFnType;
use crate::mut_input::MutInput;
use crate::grad::Grad;
pub struct Cons<'a> {
  pub func: Option<JitFunction<'a, JigsawFnType>>,
  pub comparison: u32,
  pub local_map: HashMap<u32, u32>,
  pub input_args: Vec<(bool,u64)>,
  pub inputs: HashMap<u32, u8>,
  pub shape: HashMap<u32, u32>,
  pub const_num: u32,
}

impl<'a> Cons<'a> {
  pub fn new() -> Self {
    let local_map = HashMap::new();
    let input_args = Vec::new();
    let inputs = HashMap::new();
    let shape = HashMap::new();
    Self {func:None, 
          comparison: 0, 
          local_map: local_map,
          input_args: input_args,
          inputs: inputs,
          shape: shape,
          const_num: 0,
    }
  }

  pub fn set_func(&mut self, func: JitFunction<'a, JigsawFnType>) {
      self.func = Some(func);
  }
  
  pub fn call_func(&self, x: &mut [u64]) -> u64 {
    if let Some(func) = &self.func  {
      return unsafe { func.call(x.as_mut_ptr()) };
    }
    return 0;
  }
}

//search context
pub struct SContext {
  pub min_input: MutInput,
  pub grad: Grad,
  pub distances: Vec<u64>,
  pub orig_distances: Vec<u64>,
  pub next_state: i32,
  pub step: usize,
  pub f_last: u64,
  pub dimension_idx: usize,
  pub att: usize,
  pub solved: bool,
  pub scratch_args: Vec<u64>,
}


impl SContext {
  pub fn new(input_len: usize, num_exprs: usize, scratch_input_len: usize) -> Self {
    Self {
      min_input: MutInput::new(),
      grad: Grad::new(input_len),
      distances: vec![0; num_exprs],
      orig_distances: vec![0; num_exprs],
      next_state: 0,
      step: 1,
      f_last: std::u64::MAX,
      dimension_idx: 0,
      att: 0,
      solved: false,
      scratch_args: vec![0; scratch_input_len],
    }
  }
}


pub struct Fut<'a> {
  pub num_exprs: u32,
  pub constraints: Vec<Cons<'a>>,
  pub inputs: Vec<(u32, u8)>,
  pub shape: HashMap<u32,u32>,
  pub ctx: Option<SContext>, 
  pub start_time: u64,
  pub max_const_num: u32,
  pub rgd_solutions: Vec<HashMap<u32,u8>>,
  pub opti_solutions: Vec<HashMap<u32,u8>>,
}

impl<'a> Fut<'a> {
  pub fn new() -> Self {
    Self {
      num_exprs: 0,
      constraints: Vec::new(),
      inputs: Vec::new(),
      shape: HashMap::new(),
      ctx: None,
      start_time: 0,
      max_const_num: 0,
      rgd_solutions: Vec::new(),
      opti_solutions: Vec::new(),
    }
  }
  
  pub fn finalize(&mut self) {
    let mut sym_map = HashMap::new();
    let mut gidx = 0;
    for cons in &mut self.constraints {
      for (k, v) in &cons.local_map {
        if !sym_map.contains_key(k) {
          gidx = self.inputs.len();
          sym_map.insert(*k, gidx);
          self.shape.insert(*k, cons.shape[k]);
          self.inputs.push((*k, cons.inputs[k]));
        } else {
          gidx = sym_map[k];
        }
        cons.input_args[*v as usize].1 = gidx as u64;
      }
      if self.max_const_num < cons.const_num {
        self.max_const_num = cons.const_num;
      }
    } 
    self.ctx = Some(SContext::new(self.inputs.len(), 
                        self.constraints.len(), 
                        self.inputs.len() + self.max_const_num as usize + 100));
  }

}
