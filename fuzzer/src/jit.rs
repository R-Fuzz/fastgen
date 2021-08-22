use inkwell::context::Context;
use inkwell::execution_engine::JitFunction;
use inkwell::OptimizationLevel;
use inkwell::values::FunctionValue;
use inkwell::values::IntValue;
use inkwell::builder::Builder;
use inkwell::IntPredicate;
use crate::rgd::*;
use std::collections::HashMap;
use std::sync::atomic::*;
use inkwell::AddressSpace;
use crate::op_def::*;
use num_traits::FromPrimitive;

pub const RET_OFFSET: u64 = 2;

pub type JigsawFnType = unsafe extern "C" fn(*mut u64) -> u64;
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

  fn codegen<'b>(&'b self, builder: &'b Builder, request: &AstNode, 
              local_map: &HashMap<u32,u32>, fn_val: FunctionValue<'b>,
              value_cache: &mut HashMap<u32, IntValue<'b>>) -> Option<IntValue<'b>> {

    if request.get_label() != 0 && value_cache.contains_key(&request.get_label()) {
      return Some(value_cache[&request.get_label()]);
    }

    let i32_type = self.context.i32_type();
    let i64_type = self.context.i64_type();
    let result = match FromPrimitive::from_u32(request.get_kind()) {
      Some(RGD::Bool) => {
        let bool_type = self.context.bool_type();
        if request.get_boolvalue() == 1 {
          Some(bool_type.const_int(1, false))
        } else {
          Some(bool_type.const_int(0, false))
        }
      },
      Some(RGD::Constant) => {
        let start = request.get_index();
        let length = request.get_bits() / 8; 
        let input_args  = fn_val.get_nth_param(0).unwrap().into_pointer_value();
        let idx = unsafe { builder.build_gep(input_args, &[i32_type.const_int(start as u64 + RET_OFFSET, false)], "argidx") };
        let mut ret = builder.build_load(idx, "argidx").into_int_value();
        ret = builder.build_int_truncate(ret, self.context.custom_width_int_type(request.get_bits()), "truncate");
        Some(ret)
      },
      Some(RGD::Read) => {
        let start = local_map[&request.get_index()];
        let length = request.get_bits() / 8; 
        let input_args  = fn_val.get_nth_param(0).unwrap().into_pointer_value();
        let idx = unsafe { builder.build_gep(input_args, &[i32_type.const_int(start as u64 + RET_OFFSET, false)], "argidx") };
        let mut ret = builder.build_load(idx, "argidx").into_int_value();
        for k in 1..length {
          let idx = unsafe { builder.build_gep(input_args, 
                  &[i32_type.const_int((start+k) as u64 + RET_OFFSET, false)], "argidx") };
          let mut tmp = builder.build_load(idx, "argidx").into_int_value();
          let shift_idx = i64_type.const_int((8 * k) as u64, false);
          tmp = builder.build_left_shift(tmp, shift_idx, "shl");
          ret = builder.build_int_add(ret, tmp, "add");
        }
        ret = builder.build_int_truncate(ret, self.context.custom_width_int_type(request.get_bits()), "truncate");
        Some(ret)
      },
      Some(RGD::Concat) => {
        let left = &request.get_children()[0]; 
        let right = &request.get_children()[1]; 
        let allbits = left.get_bits() + right.get_bits();
        let type_after = self.context.custom_width_int_type(allbits);
        let shift_idx = type_after.const_int(left.get_bits() as u64, false);
        let c1 = self.codegen(builder, &left, local_map, fn_val, value_cache);
        let c2 = self.codegen(builder, &right, local_map, fn_val, value_cache);
        if c1.is_some() && c2.is_some() {
        Some(builder.build_or(builder.build_left_shift(
                                      builder.build_int_z_extend(c2.unwrap(), type_after, "zext"), 
                                      shift_idx, "shl"),
                            builder.build_int_z_extend(c1.unwrap(), type_after, "zext"), "or"))
        } else {
          None
        }
      },          
      Some(RGD::Extract) => {
        let left = &request.get_children()[0]; 
        let c1 = self.codegen(builder, &left, local_map, fn_val, value_cache);
        // shift idx must be i64 to align with arugments
        if c1.is_some() {
        let type_after = self.context.custom_width_int_type(left.get_bits());
        let shift_idx = type_after.const_int(request.get_index() as u64, false);
          Some(builder.build_int_truncate(builder.build_right_shift(c1.unwrap(), shift_idx, false, "lshr"),
            self.context.custom_width_int_type(request.get_bits()), "truncate"))
        } else {
          None
        }
      },
      Some(RGD::ZExt) => {
        let left = &request.get_children()[0]; 
        let c1 = self.codegen(builder, &left, local_map, fn_val, value_cache);
        if c1.is_some() {
        let type_after = self.context.custom_width_int_type(request.get_bits());
          Some(builder.build_int_z_extend(c1.unwrap(), type_after, "zext"))
        } else {
          None
        }
      },
      Some(RGD::SExt) => {
        let left = &request.get_children()[0]; 
        let c1 = self.codegen(builder, &left, local_map, fn_val, value_cache);
        if c1.is_some() {
        let type_after = self.context.custom_width_int_type(request.get_bits());
          Some(builder.build_int_s_extend(c1.unwrap(), type_after, "sext"))
        } else {
          None
        }
      },
      Some(RGD::Add) => {
        let left = &request.get_children()[0]; 
        let right = &request.get_children()[1]; 
        let c1 = self.codegen(builder, &left, local_map, fn_val, value_cache);
        let c2 = self.codegen(builder, &right, local_map, fn_val, value_cache);
        if c1.is_some() && c2.is_some() {
         Some(builder.build_int_add(c1.unwrap(),c2.unwrap(),"add"))
        } else {
          None
        }
      },
      Some(RGD::Sub) => {
        let left = &request.get_children()[0]; 
        let right = &request.get_children()[1]; 
        let c1 = self.codegen(builder, &left, local_map, fn_val, value_cache);
        let c2 = self.codegen(builder, &right, local_map, fn_val, value_cache);
        if c1.is_some() && c2.is_some() {
          Some(builder.build_int_sub(c1.unwrap(),c2.unwrap(),"sub"))
        } else {
          None
        }
      },
      Some(RGD::Mul) => {
        let left = &request.get_children()[0]; 
        let right = &request.get_children()[1]; 
        let c1 = self.codegen(builder, &left, local_map, fn_val, value_cache);
        let c2 = self.codegen(builder, &right, local_map, fn_val, value_cache);
        if c1.is_some() && c2.is_some() {
          Some(builder.build_int_mul(c1.unwrap(),c2.unwrap(),"mul"))
        } else {
          None
        }
      },
      Some(RGD::UDiv) => {
        let left = &request.get_children()[0]; 
        let right = &request.get_children()[1]; 
        let type_after = self.context.custom_width_int_type(request.get_bits());
        let c1 = self.codegen(builder, &left, local_map, fn_val, value_cache);
        let c2 = self.codegen(builder, &right, local_map, fn_val, value_cache);
        if c1.is_some() && c2.is_some() {
          let va1 = type_after.const_int(1, false);
          let va0 = type_after.const_int(0, false);
          let cond = builder.build_int_compare(IntPredicate::EQ, c2.unwrap(), va0, "icmpeq");
          let divisor = builder.build_select(cond, va1, c2.unwrap(), "select").into_int_value();
          Some(builder.build_int_unsigned_div(c1.unwrap(),divisor,"udiv")) 
        } else {
          None
        }
      },
      Some(RGD::SDiv) => {
        let left = &request.get_children()[0]; 
        let right = &request.get_children()[1]; 
        let type_after = self.context.custom_width_int_type(request.get_bits());
        let c1 = self.codegen(builder, &left, local_map, fn_val, value_cache);
        let c2 = self.codegen(builder, &right, local_map, fn_val, value_cache);
        if c1.is_some() && c2.is_some() {
        let va1 = type_after.const_int(1, false);
        let va0 = type_after.const_zero();
        let vam1 = type_after.const_int(0x8000000000000000,false);
        let vam2 = type_after.const_int(0xFFFFFFFFFFFFFFFF,false);
        let minus2 = type_after.const_int(0x7FFFFFFFFFFFFFFE,false);
        let cond = builder.build_int_compare(IntPredicate::EQ, c2.unwrap(), va0, "icmpeq");
        let cond1 = builder.build_int_compare(IntPredicate::EQ, c1.unwrap(), vam1, "icmpeq");
        let cond2 = builder.build_int_compare(IntPredicate::EQ, c2.unwrap(), vam2, "icmpeq");
        let cond3 = builder.build_and(cond1,cond2,"land");
        let divisor = builder.build_select(cond, va1, c2.unwrap(), "select").into_int_value();
        let divisor1 = builder.build_select(cond3, minus2, divisor, "select").into_int_value();
        Some(builder.build_int_signed_div(c1.unwrap(),divisor1,"sdiv"))
        } else {
          None
        }
      },
      Some(RGD::URem) => {
        let left = &request.get_children()[0]; 
        let right = &request.get_children()[1]; 
        let type_after = self.context.custom_width_int_type(request.get_bits());
        let c1 = self.codegen(builder, &left, local_map, fn_val, value_cache);
        let c2 = self.codegen(builder, &right, local_map, fn_val, value_cache);
        if c1.is_some() && c2.is_some() {
          let va1 = type_after.const_int(1, false);
          let va0 = type_after.const_int(0, false);
          let cond = builder.build_int_compare(IntPredicate::EQ, c2.unwrap(), va0, "icmpeq");
          let divisor = builder.build_select(cond, va1, c2.unwrap(), "select").into_int_value();
          Some(builder.build_int_unsigned_rem(c1.unwrap(),divisor,"urem")) 
        } else {
          None
        }
      },
      Some(RGD::SRem) => {
        let left = &request.get_children()[0]; 
        let right = &request.get_children()[1]; 
        let type_after = self.context.custom_width_int_type(request.get_bits());
        let c1 = self.codegen(builder, &left, local_map, fn_val, value_cache);
        let c2 = self.codegen(builder, &right, local_map, fn_val, value_cache);
        if c1.is_some() && c2.is_some() {
          let va1 = type_after.const_int(1, false);
          let va0 = type_after.const_int(0, false);
          let vam1 = type_after.const_int(0x8000000000000000,false);
          let vam2 = type_after.const_int(0xFFFFFFFFFFFFFFFF,false);
          let minus2 = type_after.const_int(0xFFFFFFFFFFFFFFFE,false);
          let cond = builder.build_int_compare(IntPredicate::EQ, c2.unwrap(), va0, "icmpeq");
          let cond1 = builder.build_int_compare(IntPredicate::EQ, c1.unwrap(), vam1, "icmpeq");
          let cond2 = builder.build_int_compare(IntPredicate::EQ, c2.unwrap(), vam2, "icmpeq");
          let cond3 = builder.build_and(cond1,cond2,"land");
          let divisor = builder.build_select(cond, va1, c2.unwrap(), "select").into_int_value();
          let divisor1 = builder.build_select(cond3, minus2, divisor, "select").into_int_value();
          Some(builder.build_int_signed_rem(c1.unwrap(),divisor1,"srem"))
        } else {
          None
        }
      },
      Some(RGD::Neg) => {
        let left = &request.get_children()[0]; 
        let c1 = self.codegen(builder, &left, local_map, fn_val, value_cache);
        if c1.is_some() {
          Some(builder.build_int_neg(c1.unwrap(),"neg"))
        } else {
          None
        }
      },
      Some(RGD::Not) => {
        let left = &request.get_children()[0]; 
        let c1 = self.codegen(builder, &left, local_map, fn_val, value_cache);
        if c1.is_some() {
          Some(builder.build_not(c1.unwrap(),"neg"))
        } else {
          None
        }
      },
      Some(RGD::And) => {
        let left = &request.get_children()[0]; 
        let right = &request.get_children()[1]; 
        let type_after = self.context.custom_width_int_type(request.get_bits());
        let c1 = self.codegen(builder, &left, local_map, fn_val, value_cache);
        let c2 = self.codegen(builder, &right, local_map, fn_val, value_cache);
        if c1.is_some() && c2.is_some() {
          Some(builder.build_and(c1.unwrap(),c2.unwrap(),"and"))
        } else {
          None
        }
      },
      Some(RGD::Or) => {
        let left = &request.get_children()[0]; 
        let right = &request.get_children()[1]; 
        let type_after = self.context.custom_width_int_type(request.get_bits());
        let c1 = self.codegen(builder, &left, local_map, fn_val, value_cache);
        let c2 = self.codegen(builder, &right, local_map, fn_val, value_cache);
        if c1.is_some() && c2.is_some() {
          Some(builder.build_or(c1.unwrap(),c2.unwrap(),"or"))
        } else {
          None
        }
      },
      Some(RGD::Xor) => {
        let left = &request.get_children()[0]; 
        let right = &request.get_children()[1]; 
        let type_after = self.context.custom_width_int_type(request.get_bits());
        let c1 = self.codegen(builder, &left, local_map, fn_val, value_cache);
        let c2 = self.codegen(builder, &right, local_map, fn_val, value_cache);
        if c1.is_some() && c2.is_some() {
          Some(builder.build_xor(c1.unwrap(),c2.unwrap(),"xor"))
        } else {
          None
        }
      },
      Some(RGD::Shl) => {
        let left = &request.get_children()[0]; 
        let right = &request.get_children()[1]; 
        let c1 = self.codegen(builder, &left, local_map, fn_val, value_cache);
        let c2 = self.codegen(builder, &right, local_map, fn_val, value_cache);
        if c1.is_some() && c2.is_some() {
          Some(builder.build_left_shift(c1.unwrap(),c2.unwrap(),"shl"))
        } else {
          None
        }
      },
      Some(RGD::LShr) => {
        let left = &request.get_children()[0]; 
        let right = &request.get_children()[1]; 
        let type_after = self.context.custom_width_int_type(request.get_bits());
        let c1 = self.codegen(builder, &left, local_map, fn_val, value_cache);
        let c2 = self.codegen(builder, &right, local_map, fn_val, value_cache);
        if c1.is_some() && c2.is_some() {
          Some(builder.build_right_shift(c1.unwrap(),c2.unwrap(), false, "lshr"))
        } else {
          None
        }
      },
      Some(RGD::AShr) => {
        let left = &request.get_children()[0]; 
        let right = &request.get_children()[1]; 
        let type_after = self.context.custom_width_int_type(request.get_bits());
        let c1 = self.codegen(builder, &left, local_map, fn_val, value_cache);
        let c2 = self.codegen(builder, &right, local_map, fn_val, value_cache);
        if c1.is_some() && c2.is_some() {
          Some(builder.build_right_shift(c1.unwrap(),c2.unwrap(), true, "ashr"))
        } else {
          None
        }
      },
      //all the ICmp should be top level
      Some(RGD::Equal) | Some(RGD::Distinct) |
      Some(RGD::Ult) | Some(RGD::Ule) |
      Some(RGD::Ugt) | Some(RGD::Uge)  => {
        let left = &request.get_children()[0]; 
        let right = &request.get_children()[1]; 
        let type_after = self.context.custom_width_int_type(64);
        let c1 = self.codegen(builder, &left, local_map, fn_val, value_cache);
        let c2 = self.codegen(builder, &right, local_map, fn_val, value_cache);
        if c1.is_some() && c2.is_some() {
          let c1e = builder.build_int_z_extend(c1.unwrap(), type_after, "zext");
          let c2e = builder.build_int_z_extend(c2.unwrap(), type_after, "zext");
          let input_args  = fn_val.get_nth_param(0).unwrap().into_pointer_value();
          let idx0 = unsafe { builder.build_gep(input_args, &[i32_type.const_int(0, false)], "argidx") };
          let idx1 = unsafe { builder.build_gep(input_args, &[i32_type.const_int(1, false)], "argidx") };
          builder.build_store(idx0, c1e);
          builder.build_store(idx1, c2e);
          //we just return 0, and rely on the caller to calculate the distance
          Some(type_after.const_int(555, false))
        } else {
          None
        }
      },
      Some(RGD::Slt) | Some(RGD::Sle) |
      Some(RGD::Sgt) | Some(RGD::Sge)  => {
        let left = &request.get_children()[0]; 
        let right = &request.get_children()[1]; 
        let type_after = self.context.custom_width_int_type(64);
        let c1 = self.codegen(builder, &left, local_map, fn_val, value_cache);
        let c2 = self.codegen(builder, &right, local_map, fn_val, value_cache);
        if c1.is_some() && c2.is_some() {
          let c1e = builder.build_int_s_extend(c1.unwrap(), type_after, "zext");
          let c2e = builder.build_int_s_extend(c2.unwrap(), type_after, "zext");
          let input_args  = fn_val.get_nth_param(0).unwrap().into_pointer_value();
          let idx0 = unsafe { builder.build_gep(input_args, &[i32_type.const_int(0, false)], "argidx") };
          let idx1 = unsafe { builder.build_gep(input_args, &[i32_type.const_int(1, false)], "argidx") };
          builder.build_store(idx0, c1e);
          builder.build_store(idx1, c2e);
          //we just return 0, and rely on the caller to calculate the distance
          Some(type_after.const_int(555, false))
        } else {
          None
        }
      },
      _ => {
        debug!("Non-relational op!");
        None
        //return value_cache[&request.get_label()];
      }
    };
    
    if request.get_label() != 0 {
      value_cache.insert(request.get_label(), result.unwrap());
    }
    result
    //return value_cache[&request.get_label()];
  }

  pub fn add_function(&self, request: &AstNode, local_map: &HashMap<u32,u32>) -> Option<JitFunction<JigsawFnType>> {
    let id = self.uuid.fetch_add(1, Ordering::Relaxed);
    let module_id = format!("rgdjit_m{}", id);
    let module = self.context.create_module(&module_id);
    let i64_type = self.context.i64_type();
    let i64_pointer_type = i64_type.ptr_type(AddressSpace::Generic);
    let fn_type = i64_type.fn_type(&[i64_pointer_type.into()], false);
    let func_id = format!("rgdjit{}", id);
    let fn_val = module.add_function(&func_id, fn_type, None);
    let entry_basic_block = self.context.append_basic_block(fn_val, "entry");

    let builder = self.context.create_builder();
    builder.position_at_end(entry_basic_block);

    let mut value_cache = HashMap::new();
    if let Some(body) = self.codegen(&builder, request, local_map, fn_val, &mut value_cache) {
      //let return_instruction = builder.build_return(Some(&body.unwrap()));
      let return_instruction = builder.build_return(Some(&body));
      //dbg!("module: {:?}", module.clone());
      //dbg!("builder: {:?}", &builder);
      if !module.verify().is_ok() {
        dbg!("module: {:?}", module.clone());
        return None;
      }
      assert_eq!(return_instruction.get_num_operands(), 1);
      let execution_engine = module
        .create_jit_execution_engine(OptimizationLevel::None)
        .unwrap();
      let fun = unsafe { execution_engine.get_function(&func_id).unwrap() };
      Some(fun)
    } else {
      None
    }
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

  pub fn add_function_test(&self) -> JitFunction<JigsawFnType> {
    let id = self.uuid.fetch_add(1, Ordering::Relaxed);
    let module_id = format!("rgdjit_m{}", id);
    let func_id = format!("rgdjit{}", id);
    let module = self.context.create_module(&module_id);
    let fun = self.context.create_module(&module_id);
    let i64_type = self.context.i64_type();
    let i32_type = self.context.i32_type();
    let i64_pointer_type = i64_type.ptr_type(AddressSpace::Generic);
    let fn_type = i64_type.fn_type(&[i64_pointer_type.into()], false);

    let fn_val = module.add_function(&func_id, fn_type, None);
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
    unsafe { execution_engine.get_function(&func_id).unwrap() }
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
    let module_id = format!("rgdjit");
    let func_id = format!("rgdjitm");
    println!("test pointer load");
    let module = context.create_module(&module_id);
    let fun = context.create_module(&module_id);
    let i64_type = context.i64_type();
    let i32_type = context.i32_type();
    let i64_pointer_type = i64_type.ptr_type(AddressSpace::Generic);
    let fn_type = i64_type.fn_type(&[i64_pointer_type.into()], false);

    let fn_val = module.add_function(&func_id, fn_type, None);
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
      let add: JitFunction<JigsawFnType> =  execution_engine.get_function(&func_id).unwrap();
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
