use std::collections::HashMap;
use std::collections::HashSet;
use crate::rgd::*; 
use num_traits::FromPrimitive;
use crate::op_def::*;
use crate::util::*;
use protobuf::Message;
use protobuf::CodedInputStream;
use std::rc::Rc;
//Take a AST node as input, traverse the subtree using post-order and
//do the folollwing:
//1, For constant leaf node, let its index be the offset in the arugments (JIT function)
//2, For read leaf leaf node, build a map index->offset, where index
//is the index of the raw input, the offset is the offset in the arguments (JIT function)
//Output:
//1, AST node with index for constant node and hash field rewritten
//2, A local_map: index->offset
//3, A input_args, a vector of pairs <bool, value>. When false, it is a constant input
//      and the value is the value of the constant
//     when true, it is a read input, and the value is the offset in the global inputs
//4, A inputs: index->iv
pub fn map_args(node: &mut AstNode,
                local_map: &mut HashMap<u32,u32>, 
                shape: &mut HashMap<u32,u32>, 
                input_args: &mut Vec<(bool,u64)>, 
                inputs: &mut Vec<(u32,u8)>,
                visited: &mut HashSet<u32>,
                const_num: &mut u32,
                buf: &Vec<u8>) {
  for i in 0..node.get_children().len() {
    let c  = &mut node.mut_children()[i];
    let label = c.get_label();
    if label!=0 && visited.contains(&label) {
      continue;
    }
    visited.insert(label);
    map_args(c,local_map,shape, input_args,inputs,visited, const_num, buf);
  }


  match FromPrimitive::from_u32(node.get_kind()) {
    Some(RGD::Constant) => {
      let start = input_args.len();
      node.set_index(start as u32);
      node.set_hash(start as u32);
      let iv = node.get_value().parse::<u64>().expect("expect u64 number in value field");
      input_args.push((false,iv));
      *const_num+=1;
    },
    Some(RGD::Read) => {
/*
      let mut iv = 0;

      if !node.get_value().is_empty() {
        iv = node.get_value().parse::<u64>().expect("expect u64 number in value filed");
      }
*/
      let length = node.get_bits()/8;
      for i in 0..length {
        let offset = node.get_index() + i;
        let arg_index;
        if !local_map.contains_key(&offset) {
          arg_index = input_args.len();
          local_map.insert(offset,arg_index as u32);
          input_args.push((true,0));
          inputs.push((offset, buf[offset as usize]));
          if i == 0 {
            shape.insert(offset,length);
          }
          else {
            shape.insert(offset,0);
          }
        } else {
          arg_index = *local_map.get(&offset).unwrap() as usize;
        }
        if i==0 {
          node.set_hash(arg_index as u32);
        }
      }
    },

    _ => {
      if node.get_children().len() == 2 {
        let hash = xxhash((node.get_kind() << 16) | node.get_bits(),
            node.get_children()[0].get_hash(), node.get_children()[1].get_hash());
        node.set_hash(hash);
      }
    },
  };

}


pub fn node_fill(node: &mut AstNode,
                visited: &mut HashSet<u32>,
                node_cache: &HashMap<u32, AstNode>) {

  for i in 0..node.get_children().len() {
    let c  = &mut node.mut_children()[i];
    let label = c.get_label();
    if label!=0 && visited.contains(&label) {
      continue;
    }
    visited.insert(label);
    node_fill(c,visited, node_cache);
  }

  //fill the node
  if node.get_kind() == RGD::Uninit as u32  {
    *node = node_cache[&node.get_label()].clone();
  }

  for i in 0..node.get_children().len() {
    let c  = &mut node.mut_children()[i];
    let label = c.get_label();
    if label!=0 && visited.contains(&label) {
      continue;
    }
    visited.insert(label);
    node_fill(c,visited, node_cache);
  }
}



//e.g. equal(zext(equal(X, Y), 0))  => distinct(x,y)
pub fn simplify_clone(src: &AstNode) -> AstNode {
  //let mut dst = AstNode::new();
  let mut dst;
   

  if src.get_kind() == RGD::Not as u32 && src.get_bits() == 1 {
    let c0 = &src.get_children()[0];
    if c0.get_kind() == RGD::LOr as u32 {
      dst = c0.clone();
      dst.set_kind(RGD::LAnd as u32);
      flip_op(&mut dst.mut_children()[0]);
      flip_op(&mut dst.mut_children()[1]);
    } else if c0.get_kind() == RGD::LAnd as u32 {
      dst = c0.clone();
      dst.set_kind(RGD::LOr as u32);
      flip_op(&mut dst.mut_children()[0]);
      flip_op(&mut dst.mut_children()[1]);
    } else {
      dst = c0.clone();
      flip_op(&mut dst);
    }
    return dst; 
  }

  if src.get_kind() == RGD::Extract as u32 && src.get_bits() == 1 {
    let c0 = &src.get_children()[0];
    if c0.get_kind() == RGD::ZExt as u32 {
      let c00 = &c0.get_children()[0];
      return simplify_clone(c00);
    }
  }


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
      dst = src.clone();
      return dst;
    }

    if left.get_kind() == RGD::ZExt as u32 && right.get_kind() == RGD::Constant as u32 {
      let c00 = &left.get_children()[0];
      if is_relational(FromPrimitive::from_u32(c00.get_kind())) {
        let cv = right.get_value().parse::<u64>().expect("expect u64 number in value field");
        if src.get_kind() == RGD::Distinct as u32 {
          if cv == 0 {

            dst = c00.clone();
          } else {
            dst = c00.clone();
            flip_op(&mut dst);
          }
        } else { // RGD::Equal
          if cv == 0 {
            dst = c00.clone();
            flip_op(&mut dst);
          } else {
            dst = c00.clone();
          }
        }
      } else if c00.get_kind() == RGD::LOr as u32 {
        let cv = right.get_value().parse::<u64>().expect("expect u64 number in value field");
        if src.get_kind() == RGD::Distinct as u32 {
          if cv == 0 {
            dst = c00.clone();
          } else {
            dst = c00.clone();
            dst.set_kind(RGD::LAnd as u32);
            flip_op(&mut dst.mut_children()[0]);
            flip_op(&mut dst.mut_children()[1]);
          }
        } else { // RGD::Equal
          if cv == 0 {
            dst = c00.clone();
            dst.set_kind(RGD::LAnd as u32);
            flip_op(&mut dst.mut_children()[0]);
            flip_op(&mut dst.mut_children()[1]);
          } else {
            dst = c00.clone();
          }
        }
      } else if c00.get_kind() == RGD::LAnd as u32 {
        let cv = right.get_value().parse::<u64>().expect("expect u64 number in value field");
        if src.get_kind() == RGD::Distinct as u32 {
          if cv == 0 {
            dst = c00.clone();
          } else {
            dst = c00.clone();
            dst.set_kind(RGD::LOr as u32);
            flip_op(&mut dst.mut_children()[0]);
            flip_op(&mut dst.mut_children()[1]);
          }
        } else { // RGD::Equal
          if cv == 0 {
            dst = c00.clone();
            dst.set_kind(RGD::LOr as u32);
            flip_op(&mut dst.mut_children()[0]);
            flip_op(&mut dst.mut_children()[1]);
          } else {
            dst = c00.clone();
          }
        }
      } else {
        dst = src.clone();
      }
    } else {
      dst = src.clone();
    } 
  } else {
    dst = src.clone();
  }
  return dst;
}

pub fn flip_op(node: &mut AstNode) -> bool {
  if node.get_kind() == RGD::Constant as u32 {
    return true;
  }
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
    _ => 0,
  };
  if op != 0 {
    node.set_kind(op);
    return true;
  } else {
    return false;
  }
}

pub fn get_flipped_op(comp: u32) -> u32 {
  let op = match FromPrimitive::from_u32(comp) {
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
      _ => 0,
     // _ => panic!("Non-relational op!")
  };
  op
}


pub fn is_relational(op: Option<RGD>) -> bool {
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

pub fn is_relational_by_dfsan(op: u32) -> bool {
  if op == DFSAN_BVEQ || op == DFSAN_BVNEQ ||
    op == DFSAN_BVULT || op == DFSAN_BVULE ||
      op == DFSAN_BVUGT || op == DFSAN_BVUGE ||
      op == DFSAN_BVSLT || op == DFSAN_BVSLE ||
      op == DFSAN_BVSGT || op == DFSAN_BVSGE
  {
    true
  } else {
    false
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
      let bytes = src.write_to_bytes().unwrap();
      let mut stream = CodedInputStream::from_bytes(&bytes);
      stream.set_recursion_limit(1000);
      dst.merge_from(&mut stream).expect("merge failed");
      return;
    }

    if left.get_kind() == RGD::ZExt as u32 && right.get_kind() == RGD::Constant as u32 {
      let c00 = &left.get_children()[0];
      if is_relational(FromPrimitive::from_u32(c00.get_kind())) {
        let cv = right.get_value().parse::<u64>().expect("expect u64 number in value field");
        if src.get_kind() == RGD::Distinct as u32 {
          if cv == 0 {
            // != 0 => true => keep the same

            let bytes = c00.write_to_bytes().unwrap();
            let mut stream = CodedInputStream::from_bytes(&bytes);
            stream.set_recursion_limit(1000);
            dst.merge_from(&mut stream).expect("merge failed");
            //dst.merge_from_bytes(&c00.write_to_bytes().unwrap()).expect("merge failed");
          } else {
            // != 1 => false => negate
            let bytes = c00.write_to_bytes().unwrap();
            let mut stream = CodedInputStream::from_bytes(&bytes);
            stream.set_recursion_limit(1000);
            dst.merge_from(&mut stream).expect("merge failed");
            //      dst.merge_from_bytes(&c00.write_to_bytes().unwrap()).expect("merge failed");
            flip_op(dst);
          }
        } else { // RGD::Equal
          if cv == 0 {
            // == 0 => false => negate
            let bytes = c00.write_to_bytes().unwrap();
            let mut stream = CodedInputStream::from_bytes(&bytes);
            stream.set_recursion_limit(1000);
            dst.merge_from(&mut stream).expect("merge failed");
            //     dst.merge_from_bytes(&c00.write_to_bytes().unwrap()).expect("merge failed");
            flip_op(dst);
          } else {
            // == 1 => true => keep the same
            let bytes = c00.write_to_bytes().unwrap();
            let mut stream = CodedInputStream::from_bytes(&bytes);
            stream.set_recursion_limit(1000);
            dst.merge_from(&mut stream).expect("merge failed");
            //      dst.merge_from_bytes(&c00.write_to_bytes().unwrap()).expect("merge failed");
          }
        }
      } else {

        let bytes = src.write_to_bytes().unwrap();
        let mut stream = CodedInputStream::from_bytes(&bytes);
        stream.set_recursion_limit(1000);
        dst.merge_from(&mut stream).expect("merge failed");
        //  dst.merge_from_bytes(&src.write_to_bytes().unwrap()).expect("merge failed");
      }
    } else {
      let bytes = src.write_to_bytes().unwrap();
      let mut stream = CodedInputStream::from_bytes(&bytes);
      stream.set_recursion_limit(1000);
      dst.merge_from(&mut stream).expect("merge failed");
      //dst.merge_from_bytes(&src.write_to_bytes().unwrap()).expect("merge failed");
    } 
  } else {

    let bytes = src.write_to_bytes().unwrap();
    let mut stream = CodedInputStream::from_bytes(&bytes);
    stream.set_recursion_limit(1000);
    dst.merge_from(&mut stream).expect("merge failed");
    //dst.merge_from_bytes(&src.write_to_bytes().unwrap()).expect("merge failed");
  }
}

fn append_meta(cons: &mut Constraint, 
    local_map: &HashMap<u32,u32>, 
    shape: &HashMap<u32,u32>, 
    input_args: &Vec<(bool,u64)>,
    inputs: &Vec<(u32,u8)>,
    const_num: u32) {
  let mut meta = NodeMeta::new();
  for (&k,&v) in local_map.iter() {
    let mut amap = Mapping::new();
    amap.set_k(k);
    amap.set_v(v);
    meta.mut_map().push(amap);
  }
  for (&k,&v) in shape.iter() {
    let mut ashape = Shape::new();
    ashape.set_offset(k);
    ashape.set_start(v);
    meta.mut_shape().push(ashape);
  }
  for arg in input_args {
    let mut aarg = Arg::new();
    aarg.set_isinput(arg.0);
    aarg.set_v(arg.1);
    meta.mut_args().push(aarg);
  }
  for input in inputs {
    let mut ainput = Input::new();
    ainput.set_offset(input.0);
    ainput.set_iv(input.1 as u32);
    meta.mut_inputs().push(ainput);
  }
  meta.set_const_num(const_num);
  cons.set_meta(meta);
}


fn analyze_meta(node: &AstNode, buf: &Vec<u8>, node_cache: &HashMap<u32, AstNode>) -> Constraint {
  let mut local_map = HashMap::new();
  let mut shape = HashMap::new();
  let mut input_args = Vec::new();
  let mut inputs = Vec::new();
  let mut visited = HashSet::new();
  let mut const_num = 0;
  let mut cons = Constraint::new();
  //we also fill then node
  let mut node_copy = node.clone();
  node_fill(&mut node_copy, &mut visited, node_cache);
  //TODO simplify
  let mut node_simplify;
  if node_copy.get_kind() == RGD::Not as u32 && node_copy.get_bits() == 1 {
    flip_op(&mut node_copy.mut_children()[0]);
    node_simplify = simplify_clone(&node_copy.get_children()[0]);
  }
  else {
    node_simplify = simplify_clone(&node_copy);
  }
  let mut visited1 = HashSet::new();
  map_args(&mut node_simplify, &mut local_map, &mut shape,
            &mut input_args, &mut inputs, &mut visited1, &mut const_num, buf);
  cons.set_node(node_simplify);
  append_meta(&mut cons, &local_map, &shape, &input_args, &inputs, const_num);
  cons
}

//analyze maps and complete nodes
pub fn analyze_maps(nodes: &Vec<Vec<AstNode>>,
                    node_cache: &HashMap<u32, AstNode>,
                    buf: &Vec<u8>) -> Vec<Vec<Rc<Constraint>>> {
  let mut res = Vec::new();
  for row in nodes {
    let mut cons_row = Vec::new();
    for item in row {
      let cons = analyze_meta(item, buf, node_cache);
      cons_row.push(Rc::new(cons));
    }
    res.push(cons_row);
  } 
  res
}

//LOr of LAnds
pub fn to_dnf(node: &AstNode) -> Vec<Vec<AstNode>> {
  //print_node(node);
  let mut res = Vec::new();
  if node.get_kind() == RGD::LAnd as u32 {
    let left_list = to_dnf(&node.get_children()[0]);
    let right_list = to_dnf(&node.get_children()[1]);
    for single_left_row in &left_list {
      for single_right_row in &right_list {
        let mut combined = Vec::new();
        for item in single_left_row {
          combined.push(item.clone());
        }
        for item in single_right_row {
          combined.push(item.clone());
        }
        res.push(combined);
      }
    } 
  } else if node.get_kind() == RGD::LOr as u32  {
    let left_list = to_dnf(&node.get_children()[0]);
    let right_list = to_dnf(&node.get_children()[1]);
    for single_row in left_list {
      res.push(single_row);
    }
    for single_row in right_list {
      res.push(single_row);
    }
  } else {
    let node_copy = simplify_clone(node);
    if node_copy.get_kind() == RGD::LOr as u32  {
      let left_list = to_dnf(&node_copy.get_children()[0]);
      let right_list = to_dnf(&node_copy.get_children()[1]);
      for single_row in left_list {
        res.push(single_row);
      }
      for single_row in right_list {
        res.push(single_row);
      }
    } else if node_copy.get_kind() == RGD::LAnd as u32 {
      let left_list = to_dnf(&node_copy.get_children()[0]);
      let right_list = to_dnf(&node_copy.get_children()[1]);
      for single_left_row in &left_list {
        for single_right_row in &right_list {
          let mut combined = Vec::new();
          for item in single_left_row {
            combined.push(item.clone());
          }
          for item in single_right_row {
            combined.push(item.clone());
          }
          res.push(combined);
        }
      }
    } else {
      let mut single_row = Vec::new();
      //we are dropping constant
      if node_copy.get_kind() != RGD::Constant as u32 {
        single_row.push(node_copy.clone());
      }
      res.push(single_row);
    }
  }
  res
}

pub fn de_morgan(ori: &Vec<Vec<Rc<Constraint>>>) -> Vec<Vec<Rc<Constraint>>> {
  let mut res = Vec::new();
  if ori.len() == 0 {
    return res;
  }

  for item in &ori[0] {
      let mut row = Vec::new();
      row.push(item.clone());
      res.push(row);
  }

  if ori.len() == 1 {
    return res;
  }

  for i in 1..ori.len() {
    let cur = res;
    res = Vec::new();
    for row in cur {
      for item in &ori[i] {
        let mut new_row = row.clone();
        new_row.push(item.clone());
        res.push(new_row);
      }
    }
  }
  res
}
