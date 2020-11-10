use crate::rgd::*;
use crate::union_table::*;
use std::collections::HashSet;

fn do_uta(label: u32, ret: &mut JitRequest, table: &UnionTable, cache: &HashSet<u32>)  {
  let info = &table[label as usize];
  let mut size = info.size;
  if ( size==0 ) { 
    size = 1;
  }
}

pub fn union_to_ast(label: u32, ret: &mut JitRequest, table: &UnionTable)  {
  let mut cache = HashSet::new();
  do_uta(label,ret,table,&mut cache);
  ret.set_name("add".to_string());
  ret.set_kind(7);
  let mut left = JitRequest::new();
  let mut right = JitRequest::new();
  left.set_kind(1);
  left.set_name("constant".to_string());
  left.set_value("1".to_string());
  right.set_kind(1);
  right.set_name("constant".to_string());
  right.set_value("2".to_string());
  ret.mut_children().push(left);
  ret.mut_children().push(right);
}

