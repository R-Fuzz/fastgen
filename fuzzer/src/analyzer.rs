use std::collections::HashMap;
use std::collections::HashSet;
use crate::rgd::*; 
use num_traits::FromPrimitive;
use crate::op_def::*;
use crate::util::*;
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

