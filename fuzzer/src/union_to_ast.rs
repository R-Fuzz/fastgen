use crate::rgd::*;
use crate::op_def::*;
use crate::union_table::*;
use std::collections::HashSet;
use std::collections::HashMap;
use crate::util::*;
use crate::analyzer::*;
use num_traits::FromPrimitive;
use protobuf::Message;
use protobuf::CodedInputStream;
use fastgen_common::config;

fn do_uta(label: u32, table: &UnionTable, 
          cache: &mut HashMap<u32, HashSet<u32>>,
          node_cache: &mut HashMap<u32, AstNode>) -> Option<AstNode> {

  let info = &table[label as usize];
  let mut size = info.size;
  if size == 0 { 
    size = 1;
  }

  if cache.contains_key(&label) {
    let mut node = AstNode::new();
    node.set_label(label);
    node.set_bits(size as u32);
    return Some(node);
  }


  match info.op as u32 { DFSAN_READ => {
                    let mut ret = AstNode::new();
                    ret.set_kind(RGD::Read as u32);
                    ret.set_bits(8 as u32);
                    ret.set_index(info.op1 as u32);
                    ret.set_name("read".to_string());
                    //TODO set value field of read for iv
                    let mut deps = HashSet::new();
                    deps.insert(info.op1 as u32);
                    ret.set_label(label);
                    cache.insert(label, deps);
                    node_cache.insert(label, ret.clone());
                    return Some(ret);
                  },
    DFSAN_LOAD => {
                    let mut ret = AstNode::new();
                    ret.set_kind(RGD::Read as u32);
                    ret.set_bits(info.l2 * 8);
                    ret.set_index(table[info.l1 as usize].op1 as u32);
                    ret.set_name("read".to_string());
                    let mut deps = HashSet::new();
                    for i in 0..info.l2 as u32 {
                      deps.insert(table[info.l1 as usize].op1 as u32 + i);
                    }
                    ret.set_label(label);
                    cache.insert(label, deps);
                    node_cache.insert(label, ret.clone());
                    return Some(ret);
                  },
    DFSAN_ZEXT => {
                    let mut ret = AstNode::new();
                    ret.set_kind(RGD::ZExt as u32);
                    ret.set_bits(size as u32);
                    ret.set_name("zext".to_string());
                    if let Some(c) = do_uta(info.l1, table, cache, node_cache) {
                      ret.mut_children().push(c);
                      ret.set_label(label);
                      cache.insert(label, cache[&info.l1].clone());
                      node_cache.insert(label, ret.clone());
                      return Some(ret);
                    } else {
                      return None;
                    }
                  },
    DFSAN_SEXT => {
                    let mut ret = AstNode::new();
                    ret.set_kind(RGD::SExt as u32);
                    ret.set_bits(size as u32);
                    ret.set_name("sext".to_string());
                    if let Some(c) = do_uta(info.l1, table, cache, node_cache) {
                      ret.mut_children().push(c);
                      ret.set_label(label);
                      cache.insert(label, cache[&info.l1].clone());
                      node_cache.insert(label, ret.clone());
                      return Some(ret);
                    } else {
                      return None;
                    }
                  },
    DFSAN_TRUNC => {
                    let mut ret = AstNode::new();
                    ret.set_kind(RGD::Extract as u32);
                    ret.set_bits(size as u32);
                    ret.set_name("extract".to_string());
                    ret.set_index(0 as u32);
                    if let Some(c) = do_uta(info.l1, table, cache, node_cache) {
                      ret.mut_children().push(c);
                      ret.set_label(label);
                      cache.insert(label, cache[&info.l1].clone());
                      node_cache.insert(label, ret.clone());
                      return Some(ret);
                    } else {
                      return None;
                    }
                  },
    DFSAN_EXTRACT => {
                    let mut ret = AstNode::new();
                    ret.set_kind(RGD::Extract as u32);
                    ret.set_bits(size as u32);
                    ret.set_name("extract".to_string());
                    ret.set_index(info.op2 as u32);
                    if let Some(c) = do_uta(info.l1, table, cache, node_cache) {
                      ret.mut_children().push(c);
                      ret.set_label(label);
                      cache.insert(label, cache[&info.l1].clone());
                      node_cache.insert(label, ret.clone());
                      return Some(ret);
                    } else {
                      return None;
                    }
                  },
    DFSAN_NOT => {
                    let mut ret = AstNode::new();
                    ret.set_kind(RGD::Not as u32);
                    ret.set_bits(size as u32);
                    ret.set_name("not".to_string());
                    ret.set_index(info.op2 as u32);
                    if let Some(c) = do_uta(info.l2, table, cache, node_cache) { 
                      ret.mut_children().push(c);
                      ret.set_label(label);
                      cache.insert(label, cache[&info.l2].clone());
                      node_cache.insert(label, ret.clone());
                      return Some(ret);
                    } else {
                      return None;
                    }
                  },
    DFSAN_NEG => {
                    let mut ret = AstNode::new();
                    ret.set_kind(RGD::Neg as u32);
                    ret.set_bits(size as u32);
                    ret.set_name("neg".to_string());
                    ret.set_index(info.op2 as u32);
                    if let Some(c) = do_uta(info.l2, table, cache, node_cache) {
                      ret.mut_children().push(c);
                      ret.set_label(label);
                      cache.insert(label, cache[&info.l2].clone());
                      node_cache.insert(label, ret.clone());
                      return Some(ret);
                    } else {
                      return None;
                    }
                  },
    _ => (),
  }

  let mut left;
  let mut right;
  let mut size1: u32 = info.size as u32;
  if info.l1 >= CONST_OFFSET {
    let opt_left = do_uta(info.l1, table, cache, node_cache);
    if opt_left.is_none() {
      return None;
    } else {
      left = opt_left.unwrap();
    }
  } else {
    left = AstNode::new();
    if info.op as u32 == DFSAN_CONCAT {
      size1 = info.size as u32 - table[info.l2 as usize].size as u32;
    }
    left.set_kind(RGD::Constant as u32);
    left.set_name("constant".to_string());
    left.set_bits(size1);
    left.set_value(info.op1.to_string());
    left.set_label(0);
  }
  if info.l2 >= CONST_OFFSET {
    let opt_right = do_uta(info.l2, table, cache, node_cache);
    if opt_right.is_none() {
      return None;
    } else {
      right = opt_right.unwrap();
    }
  } else {
    right = AstNode::new();
    if info.op as u32 == DFSAN_CONCAT {
      size1 = info.size as u32 - table[info.l1 as usize].size as u32;
    }
    right.set_kind(RGD::Constant as u32);
    right.set_name("constant".to_string());
    right.set_bits(size1);
    right.set_value(info.op2.to_string());
    right.set_label(0);
  }
  let mut ret = AstNode::new();
  ret.mut_children().push(left);
  ret.mut_children().push(right);

  //TODO merge cache
  let mut merged = HashSet::new();
  if info.l1 >= CONST_OFFSET {
    for &v in &cache[&info.l1] {
      merged.insert(v);
    }
  }
  if info.l2 >= CONST_OFFSET {
    for &v in &cache[&info.l2] {
      merged.insert(v);
    }
  }
  cache.insert(label, merged);
  
  match (info.op & 0xff) as u32 {
    DFSAN_AND => {
                    if size != 1 {
                      ret.set_kind(RGD::And as u32);
                      ret.set_name("and".to_string());
                    } else {
                      ret.set_kind(RGD::LAnd as u32);
                      ret.set_name("land".to_string());
                    } 
                    ret.set_bits(size as u32);
                    ret.set_label(label);
                    node_cache.insert(label, ret.clone());
                    return Some(ret);
                  },
    DFSAN_OR => {
                    if size != 1 {
                      ret.set_kind(RGD::Or as u32);
                      ret.set_name("or".to_string());
                    } else {
                      ret.set_kind(RGD::LOr as u32);
                      ret.set_name("lor".to_string());
                    } 
                    ret.set_bits(size as u32);
                    ret.set_label(label);
                    node_cache.insert(label, ret.clone());
                    return Some(ret);
                  },
    DFSAN_XOR => {
                    ret.set_kind(RGD::Xor as u32);
                    ret.set_name("xor".to_string());
                    ret.set_bits(size as u32);
                    ret.set_label(label);
                    node_cache.insert(label, ret.clone());
                    return Some(ret);
                  },
    DFSAN_SHL => {
                    ret.set_kind(RGD::Shl as u32);
                    ret.set_name("shl".to_string());
                    ret.set_bits(size as u32);
                    ret.set_label(label);
                    node_cache.insert(label, ret.clone());
                    return Some(ret);
                  },
    DFSAN_LSHR => {
                    ret.set_kind(RGD::LShr as u32);
                    ret.set_name("lshr".to_string());
                    ret.set_bits(size as u32);
                    ret.set_label(label);
                    node_cache.insert(label, ret.clone());
                    return Some(ret);
                  },
    DFSAN_ASHR => {
                    ret.set_kind(RGD::AShr as u32);
                    ret.set_name("ashr".to_string());
                    ret.set_bits(size as u32);
                    ret.set_label(label);
                    node_cache.insert(label, ret.clone());
                    return Some(ret);
                  },
    DFSAN_ADD => {
                    ret.set_kind(RGD::Add as u32);
                    ret.set_name("add".to_string());
                    ret.set_bits(size as u32);
                    ret.set_label(label);
                    node_cache.insert(label, ret.clone());
                    return Some(ret);
                  },
    DFSAN_SUB => {
                    ret.set_kind(RGD::Sub as u32);
                    ret.set_name("sub".to_string());
                    ret.set_bits(size as u32);
                    ret.set_label(label);
                    node_cache.insert(label, ret.clone());
                    return Some(ret);
                  },
    DFSAN_MUL => {
                    ret.set_kind(RGD::Mul as u32);
                    ret.set_name("mul".to_string());
                    ret.set_bits(size as u32);
                    ret.set_label(label);
                    node_cache.insert(label, ret.clone());
                    return Some(ret);
                  },
    DFSAN_UDIV => {
                    ret.set_kind(RGD::UDiv as u32);
                    ret.set_name("udiv".to_string());
                    ret.set_bits(size as u32);
                    ret.set_label(label);
                    node_cache.insert(label, ret.clone());
                    return Some(ret);
                  },
    DFSAN_SDIV => {
                    ret.set_kind(RGD::SDiv as u32);
                    ret.set_name("sdiv".to_string());
                    ret.set_bits(size as u32);
                    ret.set_label(label);
                    node_cache.insert(label, ret.clone());
                    return Some(ret);
                  },
    DFSAN_UREM => {
                    ret.set_kind(RGD::URem as u32);
                    ret.set_name("urem".to_string());
                    ret.set_bits(size as u32);
                    ret.set_label(label);
                    node_cache.insert(label, ret.clone());
                    return Some(ret);
                  },
    DFSAN_SREM => {
                    ret.set_kind(RGD::SRem as u32);
                    ret.set_name("srem".to_string());
                    ret.set_bits(size as u32);
                    ret.set_label(label);
                    node_cache.insert(label, ret.clone());
                    return Some(ret);
                  },
    DFSAN_CONCAT => {
                    ret.set_kind(RGD::Concat as u32);
                    ret.set_name("concat".to_string());
                    ret.set_bits(size as u32);
                    ret.set_label(label);
                    node_cache.insert(label, ret.clone());
                    return Some(ret);
                  },
    _ => (),
  }
  
  match (info.op >> 8) as u32 {
    DFSAN_BVEQ => {
                    ret.set_kind(RGD::Equal as u32);
                    ret.set_name("equal".to_string());
                    ret.set_bits(size as u32);
                    ret.set_label(label);
                    node_cache.insert(label, ret.clone());
                    return Some(ret);
                  },
    DFSAN_BVNEQ => {
                    ret.set_kind(RGD::Distinct as u32);
                    ret.set_name("distinct".to_string());
                    ret.set_bits(size as u32);
                    ret.set_label(label);
                    node_cache.insert(label, ret.clone());
                    return Some(ret);
                  },
    DFSAN_BVULT => {
                    ret.set_kind(RGD::Ult as u32);
                    ret.set_name("ult".to_string());
                    ret.set_bits(size as u32);
                    ret.set_label(label);
                    node_cache.insert(label, ret.clone());
                    return Some(ret);
                  },

    DFSAN_BVULE => {
                    ret.set_kind(RGD::Ule as u32);
                    ret.set_name("ule".to_string());
                    ret.set_bits(size as u32);
                    ret.set_label(label);
                    node_cache.insert(label, ret.clone());
                    return Some(ret);
                  },
    DFSAN_BVUGT => {
                    ret.set_kind(RGD::Ugt as u32);
                    ret.set_name("ugt".to_string());
                    ret.set_bits(size as u32);
                    ret.set_label(label);
                    node_cache.insert(label, ret.clone());
                    return Some(ret);
                  },
    DFSAN_BVUGE => {
                    ret.set_kind(RGD::Uge as u32);
                    ret.set_name("uge".to_string());
                    ret.set_bits(size as u32);
                    ret.set_label(label);
                    node_cache.insert(label, ret.clone());
                    return Some(ret);
                  },
    DFSAN_BVSLT => {
                    ret.set_kind(RGD::Slt as u32);
                    ret.set_name("slt".to_string());
                    ret.set_bits(size as u32);
                    ret.set_label(label);
                    node_cache.insert(label, ret.clone());
                    return Some(ret);
                  },
    DFSAN_BVSLE => {
                    ret.set_kind(RGD::Sle as u32);
                    ret.set_name("sle".to_string());
                    ret.set_bits(size as u32);
                    ret.set_label(label);
                    node_cache.insert(label, ret.clone());
                    return Some(ret);
                  },
    DFSAN_BVSGT => {
                    ret.set_kind(RGD::Sgt as u32);
                    ret.set_name("sgt".to_string());
                    ret.set_bits(size as u32);
                    ret.set_label(label);
                    node_cache.insert(label, ret.clone());
                    return Some(ret);
                  },
    DFSAN_BVSGE => {
                    ret.set_kind(RGD::Sge as u32);
                    ret.set_name("sge".to_string());
                    ret.set_bits(size as u32);
                    ret.set_label(label);
                    node_cache.insert(label, ret.clone());
                    return Some(ret);
                  },
    _ => { return None; },

  }
}


pub fn get_one_constraint(label: u32, direction: u32, 
                    table: &UnionTable, deps: &mut HashSet<u32>, 
                    node_cache: &mut HashMap<u32, AstNode>) -> Option<AstNode> {
  let info = &table[label as usize];
  let op = (info.op >> 8) as u32;
  let mut cache = HashMap::new();
  if info.depth > config::AST_DEPTH  {
    //warn!("large tree skipped depth is {}", info.depth);
    return None;
  }
  if let Some(src) = do_uta(label, table, &mut cache, node_cache) {

    for &v in &cache[&label] {
      deps.insert(v);
    }

    return Some(src);
  } else {
    return None;
  }
}

//we do not have direction
pub fn get_addcons_constraint(label: u32, _direction: u32, 
            table: &UnionTable, deps: &mut HashSet<u32>, node_cache: &mut HashMap<u32, AstNode>) -> Option<AstNode> {
  let info = &table[label as usize];
  let op = (info.op >> 8) as u32;
  let mut cache = HashMap::new();
  if is_relational_by_dfsan(op) {
    if info.depth > config::AST_DEPTH  {
      warn!("large tree skipped depth is {}", info.depth);
      return None;
    }
    if let Some(src) = do_uta(label, table, &mut cache, node_cache) {

      for &v in &cache[&label] {
        deps.insert(v);
      }
      return Some(src);
    } else {
      return None;
    }
  } else if info.op as u32 == DFSAN_NOT {
    let info1 = &table[info.l2 as usize];
    let op1 = (info1.op >> 8) as u32;
    if is_relational_by_dfsan(op1) {
      if info.depth > config::AST_DEPTH  {
        warn!("large tree skipped depth is {}", info.depth);
        return None;
      }
      if let Some(src) = do_uta(info.l2, table, &mut cache, node_cache) {
        for &v in &cache[&info.l2] {
          deps.insert(v);
        }
        return Some(src);
      } else {
        return None;
      }
    }
  }
  None
}

pub fn get_fmemcmp_constraint(label: u32, table: &UnionTable, deps: &mut HashSet<u32>) -> (u32,usize) {
  let mut cache = HashMap::new();
  let mut node_cache = HashMap::new();

  let info = &table[label as usize];

  if info.depth > config::AST_DEPTH  {
    warn!("large tree skipped  depth is {}", info.depth);
    return (0,0);
  }
  if let Some(left) = do_uta(label, table, &mut cache, &mut node_cache) {

    for &v in &cache[&label] {
      deps.insert(v);
    }

    let mut min_v = u32::MAX;
    let len = deps.len();
    for v in deps.iter() {
      if v < &min_v {
        min_v = *v;
      }
    }

    return (min_v, len);
  } else {
    return (0,0);
  }
}

pub fn get_gep_constraint(label: u32, result: u64, table: &UnionTable, deps: &mut HashSet<u32>,
          node_cache: &mut HashMap<u32, AstNode>) -> Option<AstNode> {
  let mut cache = HashMap::new();
  let info = &table[label as usize];
  let mut right = AstNode::new();
  let mut src = AstNode::new();

  if info.depth > config::AST_DEPTH  {
    warn!("large tree skipped  depth is {}", info.depth);
    return None;
  }
  if let Some(left) = do_uta(label, table, &mut cache, node_cache) {

    //build left != result
    src.set_bits(left.get_bits() as u32);
    src.set_kind(RGD::Distinct as u32);
    src.set_name("distinct".to_string());
    right.set_kind(RGD::Constant as u32);
    right.set_name("constant".to_string());
    right.set_bits(left.get_bits() as u32);
    right.set_value(result.to_string());
    right.set_label(0);
    src.mut_children().push(left);
    src.mut_children().push(right);


    for &v in &cache[&label] {
      deps.insert(v);
    }
    return Some(src);
  } else {
    return None;
  }
}

