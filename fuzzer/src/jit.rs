use inkwell::context::Context;
use inkwell::execution_engine::JitFunction;
use inkwell::OptimizationLevel;
use crate::rgd::*;
use std::collections::HashMap;
use std::sync::atomic::*;
use inkwell::AddressSpace;

type loadaddition = unsafe extern "C" fn(*mut u64) -> u64;
type Addition = unsafe extern "C" fn(i32, i32) -> i32;

pub struct JITEngine {
  uuid: AtomicU64,
  context: Context,
  //let context = Cntext::create();
}

impl JITEngine {
  pub fn new() -> Self {
    Self {uuid: AtomicU64::new(0),  context:Context::create()}
  }
}

impl JITEngine {
  pub fn add_function(&self, request: &AstNode, local_map: &HashMap<u32,u32>) -> u64 {
    let id = self.uuid.fetch_add(1, Ordering::Relaxed);
    let moduleId = format!("rgdjit_m{}", id);
    let module = self.context.create_module(&moduleId);
    let fun = self.context.create_module(&moduleId);
    let i64_type = self.context.i64_type();
    let i64_pointer_type = i64_type.ptr_type(AddressSpace::Generic);
    let fn_type = i64_type.fn_type(&[i64_pointer_type.into()], false);
    0
  }

  pub fn add_function_add(&self) -> JitFunction<Addition> {
    let module = self.context.create_module("addition");
    let i32_type = self.context.i32_type();
    // ANCHOR_END: first
    // ANCHOR: second
    let fn_type = i32_type.fn_type(&[i32_type.into(), i32_type.into()], false);
    let fn_val = module.add_function("add", fn_type, None);
    let entry_basic_block = self.context.append_basic_block(fn_val, "entry");

    let builder = self.context.create_builder();
    builder.position_at_end(entry_basic_block);
    // ANCHOR_END: second
    // ANCHOR: third
    let x = fn_val.get_nth_param(0).unwrap().into_int_value();
    let y = fn_val.get_nth_param(1).unwrap().into_int_value();

    let ret = builder.build_int_add(x, y, "add");
    let return_instruction = builder.build_return(Some(&ret));
    // ANCHOR_END: third
    dbg!("module: {:?}", module.clone());
    dbg!("builder: {:?}", builder);
    assert_eq!(return_instruction.get_num_operands(), 1);
    let execution_engine = module
      .create_jit_execution_engine(OptimizationLevel::None)
      .unwrap();
    unsafe { execution_engine.get_function("add").unwrap() }
  }

  pub fn add_function_test(&self) -> JitFunction<loadaddition> {
    let id = self.uuid.fetch_add(1, Ordering::Relaxed);
    let moduleId = format!("rgdjit_m{}", id);
    let funcId = format!("rgdjit{}", id);
    let module = self.context.create_module(&moduleId);
    let fun = self.context.create_module(&moduleId);
    let i64_type = self.context.i64_type();
    let i32_type = self.context.i32_type();
    let i64_pointer_type = i64_type.ptr_type(AddressSpace::Generic);
    let fn_type = i64_type.fn_type(&[i64_pointer_type.into()], false);

    let fn_val = module.add_function(&funcId, fn_type, None);
    let entry_basic_block = self.context.append_basic_block(fn_val, "entry");

    let builder = self.context.create_builder();
    builder.position_at_end(entry_basic_block);
    // ANCHOR_END: second
    // ANCHOR: third
    let input  = fn_val.get_nth_param(0).unwrap().into_pointer_value();
    let idx1 = unsafe { builder.build_gep(input, &[i32_type.const_int(0, false)], "argidx") };
    let idx2  = unsafe { builder.build_gep(input, &[i32_type.const_int(1, false)], "argidx") };
    let arg1 = builder.build_load(idx1, "arg1").into_int_value();
    let arg2 = builder.build_load(idx2, "arg2").into_int_value();

    let ret = builder.build_int_add(arg1, arg2, "add");
    let return_instruction = builder.build_return(Some(&ret));
    // ANCHOR_END: third
    dbg!("module: {:?}", module.clone());
    dbg!("builder: {:?}", builder);
    assert_eq!(return_instruction.get_num_operands(), 1);
    let execution_engine = module
      .create_jit_execution_engine(OptimizationLevel::None)
      .unwrap();
    unsafe { execution_engine.get_function(&funcId).unwrap() }
  }
}


#[cfg(test)]
mod tests {
  use inkwell::context::Context;
  use inkwell::execution_engine::JitFunction;
  use inkwell::OptimizationLevel;
  use crate::jit::*;
  struct jitfunction<'a> {
    pub func: Option<JitFunction<'a, Addition>>,
  }
  impl<'a> jitfunction<'a> {
    pub fn new() -> Self {
      Self {func:None}
    }

    pub fn set_func(&mut self, func: JitFunction<'a, Addition>) {
      self.func = Some(func);
    }

    pub fn call_func(&self, x: i32, y: i32) -> i32 {
      if let Some(func) = &self.func {
        return unsafe { func.call(x,y) };
      }
      return 0;
    }
  }

#[test]
  fn test_add() {
    // ANCHOR: first
    let context = Context::create();
    let module = context.create_module("addition");
    let i32_type = context.i32_type();
    // ANCHOR_END: first
    // ANCHOR: second
    let fn_type = i32_type.fn_type(&[i32_type.into(), i32_type.into()], false);
    let fn_val = module.add_function("add", fn_type, None);
    let entry_basic_block = context.append_basic_block(fn_val, "entry");

    let builder = context.create_builder();
    builder.position_at_end(entry_basic_block);
    // ANCHOR_END: second
    // ANCHOR: third
    let x = fn_val.get_nth_param(0).unwrap().into_int_value();
    let y = fn_val.get_nth_param(1).unwrap().into_int_value();

    let ret = builder.build_int_add(x, y, "add");
    let return_instruction = builder.build_return(Some(&ret));
    // ANCHOR_END: third

    dbg!("module: {:?}", module.clone());
    dbg!("builder: {:?}", builder);
    assert_eq!(return_instruction.get_num_operands(), 1);
// ANCHOR: fourth
    let execution_engine = module
      .create_jit_execution_engine(OptimizationLevel::None)
      .unwrap();
    unsafe {
      //let add: JitFunction<Addition> = execution_engine.get_function("add").unwrap();
      let add: JitFunction<unsafe extern "C" fn(i32,i32) -> i32> = execution_engine.get_function("add").unwrap();
      let mut mystruct =  jitfunction::new();
      mystruct.set_func(add);
      let x = 10;
      let y = 1;
      for i in 1..10000000 {
        mystruct.call_func(x,y);
      }
      println!("result is {}", mystruct.call_func(x, y));
    }
    // ANCHOR_END: fourth
  }

#[test]
  fn test_pointer_load() {
    let context = Context::create();
    let moduleId = format!("rgdjit");
    let funcId = format!("rgdjitm");
    println!("test pointer load");
    let module = context.create_module(&moduleId);
    let fun = context.create_module(&moduleId);
    let i64_type = context.i64_type();
    let i32_type = context.i32_type();
    let i64_pointer_type = i64_type.ptr_type(AddressSpace::Generic);
    let fn_type = i64_type.fn_type(&[i64_pointer_type.into()], false);

    let fn_val = module.add_function(&funcId, fn_type, None);
    let entry_basic_block = context.append_basic_block(fn_val, "entry");

    let builder = context.create_builder();
    builder.position_at_end(entry_basic_block);
    // ANCHOR_END: second
    // ANCHOR: third
    let input  = fn_val.get_nth_param(0).unwrap().into_pointer_value();
    let idx1 = unsafe { builder.build_gep(input, &[i32_type.const_int(0, false)], "argidx") };
    let idx2  = unsafe { builder.build_gep(input, &[i32_type.const_int(1, false)], "argidx") };
    let arg1 = builder.build_load(idx1, "arg1").into_int_value();
    let arg2 = builder.build_load(idx2, "arg2").into_int_value();

    let ret = builder.build_int_add(arg1, arg2, "add");
    let return_instruction = builder.build_return(Some(&ret));
    // ANCHOR_END: third
    dbg!("module: {:?}", module.clone());
    dbg!("builder: {:?}", builder);
    assert_eq!(return_instruction.get_num_operands(), 1);
    let execution_engine = module
      .create_jit_execution_engine(OptimizationLevel::None)
      .unwrap();
    unsafe {
      let add: JitFunction<loadaddition> =  execution_engine.get_function(&funcId).unwrap();
      let mut x: [u64; 2] = [10, 12];
      println!("result is {}", add.call(x.as_mut_ptr()));
    }

  }

#[test]
  fn test_jitengine_pointer_load() {
      let engine = JITEngine::new();
      let fun = engine.add_function_test();
      let mut x: [u64; 2] = [10, 12];
      unsafe { println!("result is {}", fun.call(x.as_mut_ptr())); }
  }

#[test]
  fn test_jitengine_add() {
      let engine = JITEngine::new();
      let fun = engine.add_function_add();
      println!("test jitengine add");
      unsafe { println!("result is {}", fun.call(1,2)); }
  }
}
