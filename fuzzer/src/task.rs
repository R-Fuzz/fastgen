use inkwell::context::Context;
use inkwell::execution_engine::JitFunction;
use inkwell::OptimizationLevel;
use std::collections::HashMap;
use crate::jit::JigsawFnType;
use crate::mut_input::MutInput;
use crate::grad::Grad;
use std::rc::Rc;
use std::cell::RefCell;
use std::mem::transmute_copy;
pub struct Cons {
  pub func: Option<usize>,
  pub comparison: u32,
  pub local_map: HashMap<u32, u32>,
  pub input_args: Vec<(bool,u64)>,
  pub inputs: HashMap<u32, u8>,
  pub shape: HashMap<u32, u32>,
  pub const_num: u32,
}

impl Cons {
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

  pub fn set_func(&mut self, func_idx: usize) {
      self.func = Some(func_idx);
  }
  
  pub fn call_func(&self, x: &mut [u64]) -> u64 {
    unsafe {
      if let Some(func_idx) = &self.func  {
        //let func_store = gfunstore.as_ref().unwrap();
        let mut trans_func:JigsawFnType  = transmute_copy(func_idx);
        return unsafe { (trans_func)(x.as_mut_ptr()) };
      }
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


pub struct Fut {
  pub num_exprs: u32,
  pub constraints: Vec<Rc<RefCell<Cons>>>,
  pub inputs: Vec<(u32, u8)>,
  pub shape: HashMap<u32,u32>,
  pub ctx: Option<SContext>, 
  pub start_time: u64,
  pub max_const_num: u32,
  pub rgd_solutions: Vec<HashMap<u32,u8>>,
  pub opti_solutions: Vec<HashMap<u32,u8>>,
  pub cmap: HashMap<usize, Vec<usize>>,
}

impl Fut {
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
      cmap: HashMap::new(),
    }
  }
  
  pub fn finalize(&mut self) {
    let mut sym_map = HashMap::new();
    let mut gidx = 0;
    let mut cons_count = 0;
    for cons in &self.constraints {
      let mut pairs = Vec::new();
      for (k, v) in &cons.borrow().local_map {
        pairs.push((*k,*v));
      }
      for (k, v) in pairs {
        if !sym_map.contains_key(&k) {
          gidx = self.inputs.len();
          sym_map.insert(k, gidx);
          self.shape.insert(k, cons.borrow().shape[&k]);
          self.inputs.push((k, cons.borrow().inputs[&k]));
          if self.cmap.contains_key(&gidx) {
            let mut v = &mut *self.cmap.get_mut(&gidx).unwrap();
            v.push(cons_count);
          } else {
            let v = vec![cons_count];
            self.cmap.insert(gidx,v);
          }
        } else {
          gidx = sym_map[&k];
          if self.cmap.contains_key(&gidx) {
            let mut v = &mut *self.cmap.get_mut(&gidx).unwrap();
            v.push(cons_count);
          } else {
            let v = vec![cons_count];
            self.cmap.insert(gidx,v);
          }
        }
        cons.borrow_mut().input_args[v as usize].1 = gidx as u64;
      }
      cons_count += 1;
      if self.max_const_num < cons.borrow().const_num {
        self.max_const_num = cons.borrow().const_num;
      }
    } 
    self.ctx = Some(SContext::new(self.inputs.len(), 
                        self.constraints.len(), 
                        self.inputs.len() + self.max_const_num as usize + 100));
  }
}
