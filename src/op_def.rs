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


pub enum DFSAN {
  Bool = 0,
}
