use crate::protos::rgd::*;
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
  let mut name = ret.mut_name();
  name.push_str("hello");
}

