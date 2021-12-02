use std::os::unix::io::{FromRawFd, RawFd};
use crate::union_table::*;
use z3::{Solver, Config, Context, ast, Model};
use z3::ast::Ast;
use std::{fs::File, io::{self, Read}};
use std::io::BufReader;
use std::io::BufRead;
use crate::fifo::PipeMsg;
use std::collections::HashSet;
use std::collections::HashMap;
use crate::op_def::*;
use blockingqueue::BlockingQueue;
use crate::solution::*;
use crate::union_find::*;
use byteorder::{LittleEndian, ReadBytesExt};
use std::sync::{
  atomic::{AtomicBool, Ordering},
    Arc, RwLock, Mutex
};
use std::time;


pub fn serialize<'a>(label: u32, ctx: &'a Context, table: &UnionTable,
    cache: &mut HashMap<u32, HashSet<u32>>,  ops_cache: &mut HashSet<u32>,
    expr_cache: &mut HashMap<u32, z3::ast::Dynamic<'a>>) -> Option<z3::ast::Dynamic<'a>> {

  if label < 1 || label == std::u32::MAX {
    return None;
  }

  let info = &table[label as usize];

  if info.depth > 200 {
    warn!("ast tree too deep, skip solving");
    return None;
  }


  debug!("{} = (l1:{}, l2:{}, op:{}, size:{}, op1:{}, op2:{})", label,info.l1,info.l2,info.op,info.size,info.op1,info.op2);
  if expr_cache.contains_key(&label) {
    return Some(expr_cache[&label].clone())
  }

  match info.op as u32 {
    DFSAN_READ => {
      let node = ast::BV::new_const(ctx, info.op1 as u32, 8);
      expr_cache.insert(label, z3::ast::Dynamic::from(node.clone()));
      let mut deps = HashSet::new();
      deps.insert(info.op1 as u32);
      cache.insert(label, deps);
      return Some(z3::ast::Dynamic::from(node));
    },
               DFSAN_LOAD => {
                 let offset = table[info.l1 as usize].op1 as u32;
                 let mut node = ast::BV::new_const(ctx, table[info.l1 as usize].op1 as u32, 8);
                 let mut deps = HashSet::new();
                 for i in 1..info.l2 as u32 {
                   node = ast::BV::new_const(ctx, offset + i, 8).concat(&node);
                 }
                 for i in 0..info.l2 as u32 {
                   deps.insert(table[info.l1 as usize].op1 as u32 + i);
                 }
                 expr_cache.insert(label, z3::ast::Dynamic::from(node.clone()));
                 cache.insert(label, deps);
                 return Some(z3::ast::Dynamic::from(node));
               },
               DFSAN_ZEXT => {
                 let rawnode = serialize(info.l1, ctx, table, cache, ops_cache, expr_cache);
                 if let Some(node) = rawnode {
                   match node.sort_kind() {
                     z3::SortKind::Bool => {
                       let base = node.as_bool().unwrap().ite(&ast::BV::from_i64(ctx,1,1), 
                           &ast::BV::from_i64(ctx,0,1));
                       let ret = z3::ast::Dynamic::from(base.zero_ext(info.size as u32 - 1));
                       cache.insert(label, cache[&info.l1].clone());
                       expr_cache.insert(label, ret.clone());
                       return Some(ret);
                     },
                       z3::SortKind::BV => { 
                         let base = node.as_bv().unwrap();
                         cache.insert(label, cache[&info.l1].clone());
                         let ret = z3::ast::Dynamic::from(base.zero_ext(info.size as u32 - base.get_size()));
                         cache.insert(label, cache[&info.l1].clone());
                         expr_cache.insert(label, ret.clone());
                         return Some(ret);
                       },
                       _ => { return None; },
                   }
                 } else {
                   return None;
                 }
               },
               DFSAN_SEXT => {
                 let rawnode = serialize(info.l1, ctx, table, cache, ops_cache, expr_cache);
                 if let Some(node) = rawnode {
                   match node.sort_kind() {
                     z3::SortKind::Bool => {
                       let base = node.as_bool().unwrap().ite(&ast::BV::from_i64(ctx,1,1), 
                           &ast::BV::from_i64(ctx,0,1));
                       let ret = z3::ast::Dynamic::from(base.sign_ext(info.size as u32 - 1));
                       cache.insert(label, cache[&info.l1].clone());
                       expr_cache.insert(label, ret.clone());
                       return Some(ret);
                     },
                       z3::SortKind::BV => { 
                         let base = node.as_bv().unwrap();
                         let ret = z3::ast::Dynamic::from(base.sign_ext(info.size as u32 - base.get_size()));
                         cache.insert(label, cache[&info.l1].clone());
                         expr_cache.insert(label, ret.clone());
                         return Some(ret);
                       },
                       _ => { return None; },
                   }
                 } else {
                   return None;
                 }
               },
               DFSAN_TRUNC => {
                 let rawnode = serialize(info.l1, ctx, table, cache, ops_cache, expr_cache);
                 if let Some(node) = rawnode {
                   let base = node.as_bv().unwrap();
                   let ret = z3::ast::Dynamic::from(base.extract(info.size as u32 - 1, 0));
                   cache.insert(label, cache[&info.l1].clone());
                   expr_cache.insert(label, ret.clone());
                   return Some(ret);
                 } else {
                   return None;
                 }
               },
               DFSAN_EXTRACT => {
                 let rawnode = serialize(info.l1, ctx, table, cache, ops_cache, expr_cache);
                 if let Some(node) = rawnode {
                   let base = node.as_bv().unwrap();
                   let ret = z3::ast::Dynamic::from(base.extract(info.op2 as u32 + info.size as u32 - 1, info.op2 as u32));
                   cache.insert(label, cache[&info.l1].clone());
                   expr_cache.insert(label, ret.clone());
                   return Some(ret);
                 } else {
                   return None;
                 }
               },
               DFSAN_NOT => {
                 if info.l2 == 0 || info.size != 1 {
                   return None;
                 } else {
                   let rawnode = serialize(info.l2, ctx, table, cache, ops_cache, expr_cache);
                   if let Some(node) = rawnode {
                     // Only handle LNot
                     if node.sort_kind() == z3::SortKind::Bool {
                       let ret = z3::ast::Dynamic::from(node.as_bool().unwrap().not());
                       cache.insert(label, cache[&info.l2].clone());
                       expr_cache.insert(label, ret.clone());
                       return Some(ret);
                     } else {
                       return None;
                     }
                   } else {
                     return None;
                   }
                 }
               },
               DFSAN_NEG => {
                 if info.l2 == 0  {
                   return None;
                 } else {
                   let rawnode = serialize(info.l2, ctx, table, cache, ops_cache, expr_cache);
                   if let Some(node) = rawnode {
                     let ret = z3::ast::Dynamic::from(-node.as_bv().unwrap());
                     cache.insert(label, cache[&info.l2].clone());
                     expr_cache.insert(label, ret.clone());
                     return Some(ret);
                   } else {
                     return None;
                   }
                 }
               },
               _ => (),
  }


  let mut left;  
  let mut right;
  let mut size1: u32 = info.size as u32;
  if info.l1 >= 1 {
    let opt_left = serialize(info.l1, ctx, table, cache, ops_cache, expr_cache);
    if opt_left.is_none() {
      return None;
    } else {
      left = opt_left.unwrap();
    }
  } else {
    if info.op as u32 == DFSAN_CONCAT {
      size1 = info.size as u32 - table[info.l2 as usize].size as u32;
    }
    if size1 != 1 {
      left = z3::ast::Dynamic::from(ast::BV::from_i64(ctx, info.op1 as i64, size1));
    } else {
      left = z3::ast::Dynamic::from(ast::Bool::from_bool(ctx, info.op1 == 1));
    }
  }
  if info.l2 >= 1 {
    let opt_right = serialize(info.l2, ctx, table, cache, ops_cache, expr_cache);
    if opt_right.is_none() {
      return None;
    } else {
      right = opt_right.unwrap();
    }
  } else {
    if info.op as u32 == DFSAN_CONCAT {
      size1 = info.size as u32 - table[info.l1 as usize].size as u32;
    }
    if size1 != 1 {
      right = z3::ast::Dynamic::from(ast::BV::from_i64(ctx, info.op2 as i64, size1));
    } else {
      right = z3::ast::Dynamic::from(ast::Bool::from_bool(ctx, info.op2 == 1));
    }
  }

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
      if size1 != 1 {
        let node = z3::ast::Dynamic::from(left.as_bv().unwrap() & right.as_bv().unwrap());
        expr_cache.insert(label,node.clone());
        return Some(node);
      } else {
        let node = z3::ast::Dynamic::from(
            z3::ast::Bool::and(ctx,&[&left.as_bool().unwrap(),&right.as_bool().unwrap()]));
        expr_cache.insert(label,node.clone());
        return Some(node);
      }
    },
              DFSAN_OR => {
                if size1 != 1 {
                  let node = z3::ast::Dynamic::from(left.as_bv().unwrap() | right.as_bv().unwrap());
                  expr_cache.insert(label,node.clone());
                  return Some(node);
                } else {
                  let node = z3::ast::Dynamic::from(
                      z3::ast::Bool::or(ctx,&[&left.as_bool().unwrap(),&right.as_bool().unwrap()]));
                  expr_cache.insert(label,node.clone());
                  return Some(node);
                }
              },
              DFSAN_XOR => {
                let node =  z3::ast::Dynamic::from(left.as_bv().unwrap() ^ right.as_bv().unwrap());
                expr_cache.insert(label,node.clone());
                ops_cache.insert(0);
                return Some(node);
              },

              DFSAN_SHL => {
                let node = z3::ast::Dynamic::from(left.as_bv().unwrap() << right.as_bv().unwrap());
                expr_cache.insert(label,node.clone());
                return Some(node);
              },
              DFSAN_LSHR => {
                let node = z3::ast::Dynamic::from(left.as_bv().unwrap().bvlshr(&right.as_bv().unwrap()));
                expr_cache.insert(label,node.clone());
                return Some(node);
              },
              DFSAN_ASHR => {
                let node = z3::ast::Dynamic::from(left.as_bv().unwrap().bvashr(&right.as_bv().unwrap()));
                expr_cache.insert(label,node.clone());
                return Some(node);
              },
              DFSAN_ADD => {
                let node = z3::ast::Dynamic::from(left.as_bv().unwrap() + right.as_bv().unwrap());
                expr_cache.insert(label,node.clone());
                return Some(node);
              },
              DFSAN_SUB => {
                let node = z3::ast::Dynamic::from(left.as_bv().unwrap() - right.as_bv().unwrap());
                expr_cache.insert(label,node.clone());
                return Some(node);
              },
              DFSAN_MUL => {
                let node = z3::ast::Dynamic::from(left.as_bv().unwrap() * right.as_bv().unwrap());
                expr_cache.insert(label,node.clone());
                return Some(node);
              },
              DFSAN_UDIV => {
                let node = z3::ast::Dynamic::from(left.as_bv().unwrap().bvudiv(&right.as_bv().unwrap()));
                expr_cache.insert(label,node.clone());
                ops_cache.insert(1);
                return Some(node);
              },
              DFSAN_SDIV => {
                let node = z3::ast::Dynamic::from(left.as_bv().unwrap().bvsdiv(&right.as_bv().unwrap()));
                expr_cache.insert(label,node.clone());
                ops_cache.insert(2);
                return Some(node);
              },
              DFSAN_UREM => {
                let node = z3::ast::Dynamic::from(left.as_bv().unwrap().bvurem(&right.as_bv().unwrap()));
                expr_cache.insert(label,node.clone());
                ops_cache.insert(3);
                return Some(node);
              },
              DFSAN_SREM => {
                let node = z3::ast::Dynamic::from(left.as_bv().unwrap().bvsrem(&right.as_bv().unwrap()));
                expr_cache.insert(label,node.clone());
                ops_cache.insert(4);
                return Some(node);
              },
              DFSAN_CONCAT => {
                let node = z3::ast::Dynamic::from(right.as_bv().unwrap().concat(&left.as_bv().unwrap()));
                expr_cache.insert(label,node.clone());
                return Some(node);
              },
              _ => (),
  }

  match (info.op >> 8) as u32 {
    DFSAN_BVEQ => {
      let node = z3::ast::Dynamic::from(left._eq(&right));
      expr_cache.insert(label,node.clone());
      return Some(node);
    },
               DFSAN_BVNEQ => {
                 let node = z3::ast::Dynamic::from(
                     z3::ast::Ast::distinct(ctx,&[&left,&right]));
                 expr_cache.insert(label,node.clone());
                 return Some(node);
               },
               DFSAN_BVULT => {
                 let node = z3::ast::Dynamic::from(left.as_bv().unwrap().bvult(&right.as_bv().unwrap()));
                 expr_cache.insert(label,node.clone());
                 return Some(node);
               },
               DFSAN_BVULE => {
                 let node = z3::ast::Dynamic::from(left.as_bv().unwrap().bvule(&right.as_bv().unwrap()));
                 expr_cache.insert(label,node.clone());
                 return Some(node);
               },
               DFSAN_BVUGT=> {
                 let node = z3::ast::Dynamic::from(left.as_bv().unwrap().bvugt(&right.as_bv().unwrap()));
                 expr_cache.insert(label,node.clone());
                 return Some(node);
               },
               DFSAN_BVUGE=> {
                 let node = z3::ast::Dynamic::from(left.as_bv().unwrap().bvuge(&right.as_bv().unwrap()));
                 expr_cache.insert(label,node.clone());
                 return Some(node);
               },
               DFSAN_BVSLT => {
                 let node = z3::ast::Dynamic::from(left.as_bv().unwrap().bvslt(&right.as_bv().unwrap()));
                 expr_cache.insert(label,node.clone());
                 return Some(node);
               },
               DFSAN_BVSLE => {
                 let node = z3::ast::Dynamic::from(left.as_bv().unwrap().bvsle(&right.as_bv().unwrap()));
                 expr_cache.insert(label,node.clone());
                 return Some(node);
               },
               DFSAN_BVSGT=> {
                 let node = z3::ast::Dynamic::from(left.as_bv().unwrap().bvsgt(&right.as_bv().unwrap()));
                 expr_cache.insert(label,node.clone());
                 return Some(node);
               },
               DFSAN_BVSGE=> {
                 let node = z3::ast::Dynamic::from(left.as_bv().unwrap().bvsge(&right.as_bv().unwrap()));
                 expr_cache.insert(label,node.clone());
                 return Some(node);
               },
               _ => { return None; }
  }
  None
}


pub fn generate_solution(ctx: &Context, m: &Model, inputs: &HashSet<u32>) -> HashMap<u32,u8> {
  debug!("generate for {:?}", inputs);
  let mut sol = HashMap::<u32,u8>::new();
  for v in inputs {
    let test = ast::BV::new_const(&ctx, *v, 8);
    let eval = m.eval(&test.to_int(true),true).unwrap().as_i64();
    debug!("{} {:?}", v, eval.unwrap() as u8);
    sol.insert(*v, eval.unwrap() as u8);
  }   
  sol
}


pub fn solve_cond<'a>(label: u32, direction: u64, table: &UnionTable, 
    ctx: &'a Context, solver: &Solver) -> Option<HashMap<u32,u8>> {
  let result = z3::ast::Bool::from_bool(ctx, direction == 1);

  let mut ret = None;
  if label == 0 {
    return ret;
  }

  let mut cache = HashMap::new();
  let mut expr_cache = HashMap::new();
  let mut op_cache = HashSet::new();

  if op_cache.len() != 0 {
    return ret;
  }

  let rawcond = serialize(label, ctx, table, &mut cache, &mut op_cache, &mut expr_cache);

  let mut deps = HashSet::new();
  for &v in &cache[&label] {
    deps.insert(v);
  }

  if let Some(cond) = rawcond {
      if cond.as_bool().is_none() {
        error!("condition must be a bool");
        return ret;
      }
      solver.reset();
      solver.assert(&z3::ast::Ast::distinct(ctx, &[&cond, &z3::ast::Dynamic::from_ast(&result)])); 
      let mut res = solver.check();
      if res == z3::SatResult::Sat  {
        debug!("sat opt");
        let m = solver.get_model().unwrap();
        let sol_opt = generate_solution(ctx, &m, &deps);
        ret = Some(sol_opt);
      }
  }

  ret
}
