use crate::rgd::*;
use crate::op_def::RGD;
use num_traits::FromPrimitive;

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
