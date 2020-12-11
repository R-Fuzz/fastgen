use crate::rgd::*;
use crate::op_def::*;
use crate::union_table::*;
use std::collections::HashSet;
use std::collections::HashMap;
use num_traits::FromPrimitive;
use protobuf::Message;

fn do_uta(label: u32, ret: &mut AstNode, table: &UnionTable, cache: &mut HashMap<u32, HashSet<u32>>)  {
  if label==0 {
    return;
  }
  let info = &table[label as usize];
  let mut size = info.size;
  if size==0 { 
    size = 1;
  }
  if cache.contains_key(&label) {
    ret.set_label(label);
    ret.set_bits(size as u32);
    return;
  }


  match info.op as u32 {
    DFSAN_READ => {
                    ret.set_kind(RGD::Read as u32);
                    ret.set_bits(8 as u32);
                    ret.set_index(info.op1 as u32);
                    ret.set_name("read".to_string());
                    //TODO set value field of read for iv
                    let mut deps = HashSet::new();
                    deps.insert(info.op1 as u32);
                    ret.set_label(label);
                    cache.insert(label, deps);
                    return;
                  },
    DFSAN_LOAD => {
                    ret.set_kind(RGD::Read as u32);
                    ret.set_bits(info.l2 * 8);
                    ret.set_index(table[info.l1 as usize].op1 as u32);
                    ret.set_name("read".to_string());
                    let mut deps = HashSet::new();
                    for i in 0..info.l1 as u32 {
                      deps.insert(table[info.l1 as usize].op1 as u32 + i);
                    }
                    ret.set_label(label);
                    cache.insert(label, deps);
                    return;
                  },
    DFSAN_ZEXT => {
                    ret.set_kind(RGD::ZExt as u32);
                    ret.set_bits(size as u32);
                    ret.set_name("zext".to_string());
                    let mut c = AstNode::new();
                    do_uta(info.l1, &mut c, table, cache); 
                    ret.mut_children().push(c);
                    ret.set_label(label);
                    cache.insert(label, cache[&info.l1].clone());
                    return;
                  },
    DFSAN_SEXT => {
                    ret.set_kind(RGD::SExt as u32);
                    ret.set_bits(size as u32);
                    ret.set_name("sext".to_string());
                    let mut c = AstNode::new();
                    do_uta(info.l1, &mut c, table, cache); 
                    ret.mut_children().push(c);
                    ret.set_label(label);
                    cache.insert(label, cache[&info.l1].clone());
                    return;
                  },
    DFSAN_TRUNC => {
                    ret.set_kind(RGD::Extract as u32);
                    ret.set_bits(size as u32);
                    ret.set_name("extract".to_string());
                    ret.set_index(0 as u32);
                    let mut c = AstNode::new();
                    do_uta(info.l1, &mut c, table, cache); 
                    ret.mut_children().push(c);
                    ret.set_label(label);
                    cache.insert(label, cache[&info.l1].clone());
                    return;
                  },
    DFSAN_EXTRACT => {
                    ret.set_kind(RGD::Extract as u32);
                    ret.set_bits(size as u32);
                    ret.set_name("extract".to_string());
                    ret.set_index(info.op2 as u32);
                    let mut c = AstNode::new();
                    do_uta(info.l1, &mut c, table, cache); 
                    ret.mut_children().push(c);
                    ret.set_label(label);
                    cache.insert(label, cache[&info.l1].clone());
                    return;
                  },
    DFSAN_NOT => {
                    ret.set_kind(RGD::Not as u32);
                    ret.set_bits(size as u32);
                    ret.set_name("not".to_string());
                    ret.set_index(info.op2 as u32);
                    let mut c = AstNode::new();
                    do_uta(info.l2, &mut c, table, cache); 
                    ret.mut_children().push(c);
                    ret.set_label(label);
                    cache.insert(label, cache[&info.l1].clone());
                    return;
                  },
    DFSAN_NEG => {
                    ret.set_kind(RGD::Neg as u32);
                    ret.set_bits(size as u32);
                    ret.set_name("neg".to_string());
                    ret.set_index(info.op2 as u32);
                    let mut c = AstNode::new();
                    do_uta(info.l2, &mut c, table, cache); 
                    ret.mut_children().push(c);
                    ret.set_label(label);
                    cache.insert(label, cache[&info.l1].clone());
                    return;
                  },
    _ => (),
  }
  let mut left = AstNode::new();
  let mut right = AstNode::new();
  let mut size1: u32 = info.size as u32;
  if info.l1 >= CONST_OFFSET {
    do_uta(info.l1, &mut left, table, cache);
  } else {
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
    do_uta(info.l2, &mut right, table, cache);
  } else {
    if info.op as u32 == DFSAN_CONCAT {
      size1 = info.size as u32 - table[info.l1 as usize].size as u32;
    }
    right.set_kind(RGD::Constant as u32);
    right.set_name("constant".to_string());
    right.set_bits(size1);
    right.set_value(info.op2.to_string());
    right.set_label(0);
  }
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
                    return;
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
                    return;
                  },
    DFSAN_XOR => {
                    ret.set_kind(RGD::Xor as u32);
                    ret.set_name("xor".to_string());
                    ret.set_bits(size as u32);
                    ret.set_label(label);
                    return;
                  },
    DFSAN_SHL => {
                    ret.set_kind(RGD::Shl as u32);
                    ret.set_name("shl".to_string());
                    ret.set_bits(size as u32);
                    ret.set_label(label);
                    return;
                  },
    DFSAN_LSHR => {
                    ret.set_kind(RGD::LShr as u32);
                    ret.set_name("lshr".to_string());
                    ret.set_bits(size as u32);
                    ret.set_label(label);
                    return;
                  },
    DFSAN_ASHR => {
                    ret.set_kind(RGD::AShr as u32);
                    ret.set_name("ashr".to_string());
                    ret.set_bits(size as u32);
                    ret.set_label(label);
                    return;
                  },
    DFSAN_ADD => {
                    ret.set_kind(RGD::Add as u32);
                    ret.set_name("add".to_string());
                    ret.set_bits(size as u32);
                    ret.set_label(label);
                    return;
                  },
    DFSAN_SUB => {
                    ret.set_kind(RGD::Sub as u32);
                    ret.set_name("sub".to_string());
                    ret.set_bits(size as u32);
                    ret.set_label(label);
                    return;
                  },
    DFSAN_MUL => {
                    ret.set_kind(RGD::Mul as u32);
                    ret.set_name("mul".to_string());
                    ret.set_bits(size as u32);
                    ret.set_label(label);
                    return;
                  },
    DFSAN_UDIV => {
                    ret.set_kind(RGD::UDiv as u32);
                    ret.set_name("udiv".to_string());
                    ret.set_bits(size as u32);
                    ret.set_label(label);
                    return;
                  },
    DFSAN_SDIV => {
                    ret.set_kind(RGD::SDiv as u32);
                    ret.set_name("sdiv".to_string());
                    ret.set_bits(size as u32);
                    ret.set_label(label);
                    return;
                  },
    DFSAN_UREM => {
                    ret.set_kind(RGD::URem as u32);
                    ret.set_name("urem".to_string());
                    ret.set_bits(size as u32);
                    ret.set_label(label);
                    return;
                  },
    DFSAN_SREM => {
                    ret.set_kind(RGD::SRem as u32);
                    ret.set_name("srem".to_string());
                    ret.set_bits(size as u32);
                    ret.set_label(label);
                    return;
                  },
    DFSAN_CONCAT => {
                    ret.set_kind(RGD::Concat as u32);
                    ret.set_name("concat".to_string());
                    ret.set_bits(size as u32);
                    ret.set_label(label);
                    return;
                  },
    _ => (),
  }
  
  match (info.op >> 8) as u32 {
    DFSAN_BVEQ => {
                    ret.set_kind(RGD::Equal as u32);
                    ret.set_name("equal".to_string());
                    ret.set_bits(size as u32);
                    ret.set_label(label);
                    return;
                  },
    DFSAN_BVNEQ => {
                    ret.set_kind(RGD::Distinct as u32);
                    ret.set_name("distinct".to_string());
                    ret.set_bits(size as u32);
                    ret.set_label(label);
                    return;
                  },
    DFSAN_BVULT => {
                    ret.set_kind(RGD::Ult as u32);
                    ret.set_name("ult".to_string());
                    ret.set_bits(size as u32);
                    ret.set_label(label);
                    return;
                  },

    DFSAN_BVULE => {
                    ret.set_kind(RGD::Ule as u32);
                    ret.set_name("ule".to_string());
                    ret.set_bits(size as u32);
                    ret.set_label(label);
                    return;
                  },
    DFSAN_BVUGT => {
                    ret.set_kind(RGD::Ugt as u32);
                    ret.set_name("ugt".to_string());
                    ret.set_bits(size as u32);
                    ret.set_label(label);
                    return;
                  },
    DFSAN_BVUGE => {
                    ret.set_kind(RGD::Uge as u32);
                    ret.set_name("uge".to_string());
                    ret.set_bits(size as u32);
                    ret.set_label(label);
                    return;
                  },
    DFSAN_BVSLT => {
                    ret.set_kind(RGD::Slt as u32);
                    ret.set_name("slt".to_string());
                    ret.set_bits(size as u32);
                    ret.set_label(label);
                    return;
                  },
    DFSAN_BVSLE => {
                    ret.set_kind(RGD::Sle as u32);
                    ret.set_name("sle".to_string());
                    ret.set_bits(size as u32);
                    ret.set_label(label);
                    return;
                  },
    DFSAN_BVSGT => {
                    ret.set_kind(RGD::Sgt as u32);
                    ret.set_name("sgt".to_string());
                    ret.set_bits(size as u32);
                    ret.set_label(label);
                    return;
                  },
    DFSAN_BVSGE => {
                    ret.set_kind(RGD::Sge as u32);
                    ret.set_name("sge".to_string());
                    ret.set_bits(size as u32);
                    ret.set_label(label);
                    return;
                  },
    _ => (),

  }

}

fn flip_op(node: &mut AstNode) {
  let op = match FromPrimitive::from_u32(node.get_kind()) {
    Some(RGD::Equal) => RGD::Distinct as u32,
    Some(RGD::Distinct) => RGD::Equal as u32,
    Some(RGD::Sge) => RGD::Slt as u32,
    Some(RGD::Sgt) => RGD::Sle as u32,
    Some(RGD::Sle) => RGD::Sgt as u32,
    Some(RGD::Slt) => RGD::Sge as u32,
    Some(RGD::Uge) => RGD::Ult as u32,
    Some(RGD::Ugt) => RGD::Ule as u32,
    Some(RGD::Ule) => RGD::Ugt as u32,
    Some(RGD::Ult) => RGD::Uge as u32,
    _ => panic!("Non-relational op!")
  };
  node.set_kind(op);
}


fn is_relational(op: Option<RGD>) -> bool {
  match op {
    Some(RGD::Equal) => true,
    Some(RGD::Distinct) => true,
    Some(RGD::Sgt) => true,
    Some(RGD::Sge) => true,
    Some(RGD::Sle) => true,
    Some(RGD::Slt) => true,
    Some(RGD::Uge) => true,
    Some(RGD::Ugt) => true,
    Some(RGD::Ule) => true,
    Some(RGD::Ult) => true,
    _ => false,
  }
}

//e.g. equal(zext(equal(X, Y), 0))  => distinct(x,y)
fn simplify(src: &mut AstNode, dst: &mut AstNode) {

  if src.get_kind() == RGD::Distinct as u32 || src.get_kind() == RGD::Equal as u32 {
    let c0 = &src.get_children()[0];
    let c1 = &src.get_children()[1];

    let left;
    let right;
    if c1.get_kind() == RGD::ZExt as u32 && c0.get_kind() == RGD::Constant as u32 {
      left = c1;
      right = c0;
    } else if c0.get_kind() == RGD::ZExt as u32 && c1.get_kind() == RGD::Constant as u32 {
      left = c0;
      right = c1;
    } else {
      dst.merge_from_bytes(&src.write_to_bytes().unwrap()).expect("merge failed");
      return;
    }

    if left.get_kind() == RGD::ZExt as u32 && right.get_kind() == RGD::Constant as u32 {
      let c00 = &left.get_children()[0];
      if is_relational(FromPrimitive::from_u32(c00.get_kind())) {
        let cv = right.get_value().parse::<u64>().expect("expect u64 number in value field");
        if src.get_kind() == RGD::Distinct as u32 {
          if cv == 0 {
            // != 0 => true => keep the same
            dst.merge_from_bytes(&c00.write_to_bytes().unwrap()).expect("merge failed");
          } else {
            // != 1 => false => negate
            dst.merge_from_bytes(&c00.write_to_bytes().unwrap()).expect("merge failed");
            flip_op(dst);
          }
        } else { // RGD::Equal
          if cv == 0 {
            // == 0 => false => negate
            dst.merge_from_bytes(&c00.write_to_bytes().unwrap()).expect("merge failed");
            flip_op(dst);
          } else {
            // == 1 => true => keep the same
            dst.merge_from_bytes(&c00.write_to_bytes().unwrap()).expect("merge failed");
          }
        }
      } else {
        dst.merge_from_bytes(&src.write_to_bytes().unwrap()).expect("merge failed");
      }
    } else {
      dst.merge_from_bytes(&src.write_to_bytes().unwrap()).expect("merge failed");
    } 
  } else {
      dst.merge_from_bytes(&src.write_to_bytes().unwrap()).expect("merge failed");
  }
}

pub fn get_one_constraint(label: u32, direction: u32, dst: &mut AstNode,  table: &UnionTable) {
  let info = &table[label as usize];
  let op = (info.op >> 8) as u32;
  let mut cache = HashMap::new();
  assert!(op == DFSAN_BVEQ || op == DFSAN_BVNEQ ||
          op == DFSAN_BVULT || op == DFSAN_BVULE ||
          op == DFSAN_BVUGT || op == DFSAN_BVUGE ||
          op == DFSAN_BVSLT || op == DFSAN_BVSLE ||
          op == DFSAN_BVSGT || op == DFSAN_BVSGE, "the operator is not relational {}", info.op); 

  let mut src = AstNode::new();
  do_uta(label, &mut src, table, &mut cache);
  if direction == 1 {
    flip_op(&mut src);
  }
  
  simplify(&mut src, dst);
}

