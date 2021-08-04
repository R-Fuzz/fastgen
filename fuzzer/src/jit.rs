use inkwell::context::Context;
use inkwell::execution_engine::JitFunction;
use inkwell::OptimizationLevel;
use crate::rgd::*;
use std::collections::HashMap;
use std::sync::atomic::*;
use inkwell::AddressSpace;

pub struct JITEngine {
  uuid: AtomicU64,
}

impl JITEngine {
  pub fn add_function(&self, request: &AstNode, local_map: &HashMap<u32,u32>) -> u64 {
    let id = self.uuid.fetch_add(1, Ordering::Relaxed);
    let moduleId = format!("rgdjit_m{}", id);
    let context = Context::create();
    let module = context.create_module(&moduleId);
    let fun = context.create_module(&moduleId);
    let i64_type = context.i64_type();
    let i64_pointer_type = i64_type.ptr_type(AddressSpace::Generic);
    let fn_type = i64_type.fn_type(&[i64_pointer_type.into()], false);
    0
  }

  pub fn add_function_test(&self) -> u64 {
    let id = self.uuid.fetch_add(1, Ordering::Relaxed);
    let moduleId = format!("rgdjit_m{}", id);
    let funcId = format!("rgdjit{}", id);
    let context = Context::create();
    let module = context.create_module(&moduleId);
    let fun = context.create_module(&moduleId);
    let i64_type = context.i64_type();
    let i64_pointer_type = i64_type.ptr_type(AddressSpace::Generic);
    let fn_type = i64_type.fn_type(&[i64_pointer_type.into()], false);

    let fn_val = module.add_function(&funcId, fn_type, None);
    let entry_basic_block = context.append_basic_block(fn_val, "entry");

    let builder = context.create_builder();
    builder.position_at_end(entry_basic_block);
    // ANCHOR_END: second
    // ANCHOR: third
    let x = fn_val.get_nth_param(0).unwrap().into_pointer_value();
    

    let ret = builder.build_int_signed_div(x, y, "add");
    let return_instruction = builder.build_return(Some(&ret));
    // ANCHOR_END: third
    dbg!("module: {:?}", module.clone());
    dbg!("builder: {:?}", builder);
    assert_eq!(return_instruction.get_num_operands(), 1);

    0
  }
}

#[cfg(test)]
mod tests {
  use inkwell::context::Context;
  use inkwell::execution_engine::JitFunction;
  use inkwell::OptimizationLevel;
  type Addition = unsafe extern "C" fn(i32, i32) -> i32;
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
  fn test_jit() {
    // ANCHOR: first
      println!("test_jit");
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

    let ret = builder.build_int_signed_div(x, y, "add");
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
}
