use crate::rgd::*;
use crate::op_def::RGD;
use num_derive::FromPrimitive;    
use num_traits::FromPrimitive;

pub fn do_print(req: &JitRequest) {
  print!("{}(",req.get_name());
  print!("width={},",req.get_bits());
  print!("label={},",req.get_label());
  match FromPrimitive::from_u32(req.get_kind()) {
    Some(RGD::Bool) => print!("{}",req.get_value()),
    Some(RGD::Constant) => print!("{},",req.get_value()),
    Some(RGD::Memcmp) => print!("{},",req.get_value()),
    Some(RGD::Read) => print!("{},",req.get_index()),
    Some(RGD::Extract) => print!("{},",req.get_index()),
    None => (),
  }
  for c in req.get_children() {
    do_print(c);
    print!(", ");
  }
  print!(")");
}

pub fn printReq(req: &JitRequest) {
  do_print(req);
  println!("");
}
