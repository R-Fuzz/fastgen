use num_derive::FromPrimitive;    
use num_traits::FromPrimitive;

#[derive(FromPrimitive)]
pub enum RGD {
  Bool = 0,
  Constant,
  Read,
  Concat,
  Extract,
  ZExt,
  SExt,

  //Arithmetic
  Add,
  Sub,
  Mul,
  UDiv,
  SDiv,
  URem,
  SRem,
  Neg,

  //Bitwise
  Not,
  And,
  Or,
  Xor,
  Shl,
  LShr,
  AShr,

  // Relational
  Equal,
  Distinct,
  Ult,
  Ule,
  Ugt,
  Uge,
  Slt,
  Sle,
  Sgt,
  Sge,
  
  //Logical
  LOr,
  LAnd,
  LNot,

  //Special
  Ite,
  Load,
  Memcmp,
}

//Derived from llvm-6.0/llvm/IR/Instruction.def 
//and dfsan.h
pub const DFSAN_Read: u32 = 0;
pub const DFSAN_Not: u32 = 1;
pub const DFSAN_Neg: u32 = 2;
pub const DFSAN_Add: u32 = 11;
pub const DFSAN_Sub: u32 = 13;
pub const DFSAN_Mul: u32 = 15;
pub const DFSAN_UDiv: u32 = 17;
pub const DFSAN_SDiv: u32 = 18;
pub const DFSAN_URem: u32 = 20;
pub const DFSAN_SRem: u32 = 21;
pub const DFSAN_Shl: u32 = 23;
pub const DFSAN_LShr: u32 = 24;
pub const DFSAN_AShr: u32 = 25;
pub const DFSAN_And: u32 = 26;
pub const DFSAN_Or: u32 = 27;
pub const DFSAN_Xor: u32 = 28;
pub const DFSAN_Trunc: u32 = 36;
pub const DFSAN_ZExt: u32 = 37;
pub const DFSAN_SExt: u32 = 38;
pub const DFSAN_Load: u32 = 67;
pub const DFSAN_Extract: u32 = 68;
pub const DFSAN_Concat: u32 = 69;
//relational
pub const DFSAN_bveq: u32 = 32;
pub const DFSAN_bvneq: u32 = 33;
pub const DFSAN_bvugt: u32 = 34;
pub const DFSAN_bvuge: u32 = 35;
pub const DFSAN_bvult: u32 = 36;
pub const DFSAN_bvule: u32 = 37;
pub const DFSAN_bvsgt: u32 = 38;
pub const DFSAN_bvsge: u32 = 39;
pub const DFSAN_bvslt: u32 = 40;
pub const DFSAN_bvsle: u32 = 41;
