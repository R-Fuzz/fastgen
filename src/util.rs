use crate::rgd::*;
use crate::op_def::*;
use num_traits::FromPrimitive;

pub fn to_rgd_op(op: u32) -> u32 {
  match op {
    DFSAN_BVEQ => RGD::Equal as u32,
    DFSAN_BVNEQ => RGD::Distinct as u32,
    DFSAN_BVSGT => RGD::Sgt as u32,
    DFSAN_BVSGE => RGD::Sge as u32,
    DFSAN_BVSLT => RGD::Slt as u32,
    DFSAN_BVSLE => RGD::Sle as u32,
    DFSAN_BVUGT => RGD::Ugt as u32,
    DFSAN_BVUGE => RGD::Uge as u32,
    DFSAN_BVULT => RGD::Ult as u32,
    DFSAN_BVULE => RGD::Ule as u32,
    _ => 0,
  }
}

pub fn do_print(node: &AstNode) {
  print!("{}(",node.get_name());
  print!("width={},",node.get_bits());
  print!("label={},",node.get_label());
  match FromPrimitive::from_u32(node.get_kind()) {
    Some(RGD::Bool) => print!("{}",node.get_value()),
    Some(RGD::Constant) => print!("{},",node.get_value()),
    Some(RGD::Memcmp) => print!("{},",node.get_value()),
    Some(RGD::Read) => print!("{},",node.get_index()),
    Some(RGD::Extract) => print!("{},",node.get_index()),
    _ => (),
  }
  for c in node.get_children() {
    do_print(c);
    print!(", ");
  }
  print!(")");
}

pub fn print_node(node: &AstNode) {
  do_print(node);
  println!("");
}

pub fn print_task(task: &SearchTask) {
  for cons in task.get_constraints() {
    println!("Op is {}", cons.get_comparison());
    print_node(cons.get_left());
    print_node(cons.get_right());
  }
}
