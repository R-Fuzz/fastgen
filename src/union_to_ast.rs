use crate::protos::rgd::*;

pub fn union_to_ast(label: u32, ret: &mut JitRequest)  {
  let mut name = ret.mut_name();
  name.push_str("hello");
}

