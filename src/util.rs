use crate::rgd::*;
use crate::op_def::*;
use num_traits::FromPrimitive;

pub fn to_rgd_op(op: u32) -> u32 {
  match op {
    DFSAN_BVEQ => RGD::Equal as u32,
    DFSAN_BVSGT => RGD::Sgt as u32,
    _ => 0,
  }
}

pub fn do_print(node: &RealAstNode) {
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

pub fn print_node(node: &RealAstNode) {
  do_print(node);
  println!("");
}

pub fn print_v_node(node: &AstNode) {
  if node.get_virt() {
    println!("label is {}", node.get_label());
  } else {
    do_print(node.get_payload());
  }
  println!("");

}
