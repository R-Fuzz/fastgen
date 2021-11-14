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

#[derive(Clone)]
pub struct BranchDep<'a> {
  pub cons_set: Vec<z3::ast::Dynamic<'a>>,
}


fn union(uf: &mut UnionFind, inputs: &HashSet<u32>) -> u32 {
  //UnionFind union
  let mut init = false;
  let mut v0 = 0;
  for &v in inputs.iter() {
    if !init {
      v0 = v;
      init = true;
    }
    uf.union(v as usize, v0 as usize);
  }
  v0
}


pub fn serialize<'a>(label: u32, ctx: &'a Context, table: &UnionTable,
    cache: &mut HashMap<u32, HashSet<u32>>, 
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
                 let rawnode = serialize(info.l1, ctx, table, cache, expr_cache);
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
                 let rawnode = serialize(info.l1, ctx, table, cache, expr_cache);
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
                 let rawnode = serialize(info.l1, ctx, table, cache, expr_cache);
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
                 let rawnode = serialize(info.l1, ctx, table, cache, expr_cache);
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
                   let rawnode = serialize(info.l2, ctx, table, cache, expr_cache);
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
                   let rawnode = serialize(info.l2, ctx, table, cache, expr_cache);
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
    let opt_left = serialize(info.l1, ctx, table, cache, expr_cache);
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
    let opt_right = serialize(info.l2, ctx, table, cache, expr_cache);
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
                return Some(node);
              },
              DFSAN_SDIV => {
                let node = z3::ast::Dynamic::from(left.as_bv().unwrap().bvsdiv(&right.as_bv().unwrap()));
                expr_cache.insert(label,node.clone());
                return Some(node);
              },
              DFSAN_UREM => {
                let node = z3::ast::Dynamic::from(left.as_bv().unwrap().bvurem(&right.as_bv().unwrap()));
                expr_cache.insert(label,node.clone());
                return Some(node);
              },
              DFSAN_SREM => {
                let node = z3::ast::Dynamic::from(left.as_bv().unwrap().bvsrem(&right.as_bv().unwrap()));
                expr_cache.insert(label,node.clone());
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

pub fn add_cons<'a>(label: u32, table: &UnionTable, 
    ctx: &'a Context, solver: &Solver, 
    uf: &mut UnionFind, branch_deps: &mut Vec<Option<BranchDep<'a>>>) {


  if label == 0 {
    return;
  }
  let info = &table[label as usize];

  let mut cache = HashMap::new();
  let mut expr_cache = HashMap::new();

  let rawcond = serialize(label, ctx, table, &mut cache, &mut expr_cache);


  if let Some(cond) = rawcond {

    let mut deps = HashSet::new();
    for &v in &cache[&label] {
      deps.insert(v);
    } 

    let v0 = union(uf, &deps) as usize;

    if cond.as_bool().is_none() {
      error!("condition must be a bv for gep");
      return;
    }
    //preserve dependencies
    //preserve
    preserve(cond.as_bool().unwrap(), v0, branch_deps);
  }
  return;
}


pub fn solve_fmemcmp(label: u32, data: &Vec<u8>, size: u64, try_solve: bool, table: &UnionTable, 
    ctx: &Context, solver: &Solver) -> Option<HashMap<u32,u8>> {


  let mut ret = None;
  if label == 0 {
    return ret;
  }

  let mut cache = HashMap::new();
  let mut expr_cache = HashMap::new();

  let rawcond = serialize(label, ctx, table, &mut cache, &mut expr_cache);


  if let Some(cond) = rawcond {

    let mut deps = HashSet::new();
    for &v in &cache[&label] {
      deps.insert(v);
    } 

    if try_solve {
      if cond.as_bv().is_none() {
        error!("condition must be a bv for gep");
        return ret;
      }
      let mut op_concrete  = ast::BV::from_u64(ctx, data[0] as u64, 8);
      for i in 1..data.len() {
        op_concrete = ast::BV::from_u64(ctx, data[i] as u64, 8).concat(&op_concrete);
      }
      solver.reset();
      solver.assert(&cond._eq(&z3::ast::Dynamic::from_ast(&op_concrete)));
      let mut res = solver.check();
      if res == z3::SatResult::Sat  {
        debug!("sat opt");
        let m = solver.get_model().unwrap();
        let sol_opt = generate_solution(ctx, &m, &deps);
        ret = Some(sol_opt);
      } else {
        debug!("not sat fmemcmp");
      }
    }
  }

  ret
}

pub fn solve_gep<'a>(label: u32, result: u64, try_solve: bool, table: &UnionTable, 
    ctx: &'a Context, solver: &Solver, 
    uf: &mut UnionFind, branch_deps: &mut Vec<Option<BranchDep<'a>>>) -> (Option<HashMap<u32,u8>>, Option<HashMap<u32,u8>>) {


  let mut ret = (None, None);
  if label == 0 {
    return ret;
  }

  let info = &table[label as usize];

  let result = z3::ast::BV::from_u64(ctx,result,info.size as u32);

  let mut cache = HashMap::new();
  let mut expr_cache = HashMap::new();

  let rawcond = serialize(label, ctx, table, &mut cache, &mut expr_cache);


  if let Some(cond) = rawcond {

    let mut deps = HashSet::new();
    for &v in &cache[&label] {
      deps.insert(v);
    } 

    let v0 = union(uf, &deps) as usize;

    if try_solve {
      if cond.as_bv().is_none() {
        error!("condition must be a bv for gep");
        return ret;
      }
      solver.reset();
      solver.assert(&cond._eq(&z3::ast::Dynamic::from_ast(&result)));
      debug!("{:}", solver);
      let mut res = solver.check();
      if res == z3::SatResult::Sat  {
        debug!("sat opt");
        let m = solver.get_model().unwrap();
        let sol_opt = generate_solution(ctx, &m, &deps);
        ret.0 = Some(sol_opt);
        solver.push(); 
        add_dependencies(solver, v0, uf, branch_deps);
        res = solver.check();
        if res == z3::SatResult::Sat  {
          debug!("sat opt");
          let m = solver.get_model().unwrap();
          let sol_nest = generate_solution(ctx, &m, &deps);
          ret.1 = Some(sol_nest);
        }
      }
    }
    //preserve dependencies
    //preserve
    let path_cond = z3::ast::Ast::distinct(ctx, &[&cond, &z3::ast::Dynamic::from_ast(&result)]); 
    preserve(path_cond, v0, branch_deps);
  }

  ret
}

pub fn solve_cond<'a>(label: u32, direction: u64, try_solve: bool, table: &UnionTable, 
    ctx: &'a Context, solver: &Solver, 
    uf: &mut UnionFind, branch_deps: &mut Vec<Option<BranchDep<'a>>>) -> (Option<HashMap<u32,u8>>, Option<HashMap<u32,u8>>) {
  let result = z3::ast::Bool::from_bool(ctx, direction == 1);

  let mut ret = (None, None);
  if label == 0 {
    return ret;
  }

  let mut cache = HashMap::new();
  let mut expr_cache = HashMap::new();

  let rawcond = serialize(label, ctx, table, &mut cache, &mut expr_cache);


  if let Some(cond) = rawcond {

    let mut deps = HashSet::new();
    for &v in &cache[&label] {
      deps.insert(v);
    } 

    let v0 = union(uf, &deps) as usize;

    if try_solve {
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
        ret.0 = Some(sol_opt);
        solver.push(); 
        add_dependencies(solver, v0, uf, branch_deps);
        res = solver.check();
        if res == z3::SatResult::Sat  {
          debug!("sat opt");
          let m = solver.get_model().unwrap();
          let sol_nest = generate_solution(ctx, &m, &deps);
          ret.1 = Some(sol_nest);
        }
      }
    }
    //preserve dependencies
    //preserve
    let path_cond = cond._eq(&z3::ast::Dynamic::from_ast(&result));
    preserve(path_cond, v0, branch_deps);
  }

  ret
}

fn preserve<'a>(cond: z3::ast::Bool<'a>, v0: usize, branch_deps: &mut Vec<Option<BranchDep<'a>>>) {
  //add to nested dependency tree
  let mut is_empty = false;
  {
    let deps_opt = &branch_deps[v0 as usize];
    if deps_opt.is_none() {
      is_empty = true;
    }
  }
  if is_empty {
    branch_deps[v0 as usize] = Some(BranchDep {cons_set: Vec::new()});
  }
  let deps_opt = &mut branch_deps[v0 as usize];
  let deps = deps_opt.as_mut().unwrap();
  deps.cons_set.push(z3::ast::Dynamic::from(cond));
}

fn add_dependencies(solver: &Solver, v0: usize, uf: &mut UnionFind, branch_deps: &mut Vec<Option<BranchDep>>) {
  for off in uf.get_set(v0 as usize) {
    let deps_opt = &branch_deps[off as usize];
    if let Some(deps) = deps_opt {
      for cons in &deps.cons_set {
        solver.assert(&cons.as_bool().unwrap());
      }
    }
  }
}

pub fn solve(shmid: i32, pipefd: RawFd, solution_queue: BlockingQueue<Solution>, tainted_size: usize,
    branch_gencount: &Arc<RwLock<HashMap<(u64,u64,u32,u64), u32>>>,
    branch_fliplist: &Arc<RwLock<HashSet<(u64,u64,u32,u64)>>>,
    branch_hitcount: &Arc<RwLock<HashMap<(u64,u64,u32,u64), u32>>>) {
  info!("solve shmid {} and pipefd {}", shmid, pipefd);
  let rawptr = unsafe { libc::shmat(shmid, std::ptr::null(), 0) };
  let ptr = unsafe { rawptr as *mut UnionTable};
  let table = unsafe { & *ptr };
  let mut cfg = Config::new();  
  cfg.set_timeout_msec(10000);
  let ctx = Context::new(&cfg);
  let solver = Solver::new(&ctx);
  let f = unsafe { File::from_raw_fd(pipefd) }; 
  let mut branch_deps : Vec<Option<BranchDep>> = vec![None;tainted_size];
  let mut uf = UnionFind::<usize>::new(tainted_size);
  let mut reader = BufReader::new(f);
  let t_start = time::Instant::now();
  let mut branch_local = HashMap::<(u64,u64,u64),u32>::new();
  loop {
    let rawmsg = PipeMsg::from_reader(&mut reader);
    if let Ok(msg) = rawmsg {

      let mut hitcount = 1;
      let mut gencount = 0;
      let mut flipped = false;
      let mut localcnt = 1;

      if branch_local.contains_key(&(msg.addr,msg.ctx,msg.result)) {
        localcnt = *branch_local.get(&(msg.addr,msg.ctx,msg.result)).unwrap();
        localcnt += 1;
      }
      branch_local.insert((msg.addr,msg.ctx,msg.result),localcnt);

      debug!("tid: {} label: {} result: {} addr: {} ctx: {} localcnt: {} type: {}",
          msg.tid, msg.label, msg.result, msg.addr, msg.ctx, localcnt, msg.msgtype);

      if branch_hitcount.read().unwrap().contains_key(&(msg.addr,msg.ctx,localcnt,msg.result)) {
        hitcount = *branch_hitcount.read().unwrap().get(&(msg.addr,msg.ctx,localcnt,msg.result)).unwrap();
        hitcount += 1;
      }
      branch_hitcount.write().unwrap().insert((msg.addr,msg.ctx,localcnt,msg.result), hitcount);

      if branch_fliplist.read().unwrap().contains(&(msg.addr,msg.ctx,localcnt,msg.result)) {
        //info!("the branch is flipped");
        flipped = true;
      }

      if branch_gencount.read().unwrap().contains_key(&(msg.addr,msg.ctx,localcnt,msg.result)) {
        gencount = *branch_gencount.read().unwrap().get(&(msg.addr,msg.ctx,localcnt,msg.result)).unwrap();
      }

      if msg.msgtype == 0 {
        if localcnt > 64 { continue; }
        let try_solve = hitcount <= 5 && (!flipped) && gencount == 0;
        let rawsol = solve_cond(msg.label, msg.result, try_solve, &table, &ctx, &solver, &mut uf, &mut branch_deps);
        if let Some(sol) = rawsol.0 {
          let sol_size = sol.len();
          let rgd_sol = Solution::new(sol, msg.tid, msg.addr, msg.ctx,
              localcnt,  msg.result, 0, sol_size, msg.bid, msg.sctx, true, msg.predicate, msg.target_cond);
          solution_queue.push(rgd_sol);
        }
        if let Some(sol) = rawsol.1 {
          let sol_size = sol.len();
          let rgd_sol = Solution::new(sol, msg.tid, msg.addr, msg.ctx,
              localcnt,  msg.result, 0, sol_size, msg.bid, msg.sctx, true, msg.predicate, msg.target_cond);
          solution_queue.push(rgd_sol);
        }
      } else if msg.msgtype == 1 {
        //gep
        if localcnt > 64 { continue; }
        let try_solve = hitcount <= 5;
        let rawsol = solve_gep(msg.label, msg.result, try_solve, &table, &ctx, &solver, &mut uf, &mut branch_deps);
        if let Some(sol) = rawsol.0 {
          let sol_size = sol.len();
          let rgd_sol = Solution::new(sol, msg.tid, msg.addr, msg.ctx,
                localcnt,  msg.result, 0, sol_size, msg.bid, msg.sctx, false, 0, 0);
          solution_queue.push(rgd_sol);
        }
        if let Some(sol) = rawsol.1 {
          let sol_size = sol.len();
          let rgd_sol = Solution::new(sol, msg.tid, msg.addr, msg.ctx,
              localcnt,  msg.result, 0, sol_size, msg.bid, msg.sctx, false, 0, 0);
          solution_queue.push(rgd_sol);
        }
      } else if msg.msgtype == 2 {
        //strcmp
        let mut data = Vec::new();
        for _i in 0..msg.result as usize {
          if let Ok(cur) = reader.read_u8() {
            data.push(cur);
          } else {
            break;
          }
        }
        if data.len() < msg.result as usize {
          break;
        }
        if localcnt > 64 { continue; }
        let try_solve = hitcount <=5;
        let rawsol = solve_fmemcmp(msg.label, &data, msg.result, try_solve, &table, &ctx, &solver);
        if let Some(sol) = rawsol {
          let sol_size = sol.len();
          let rgd_sol = Solution::new(sol, msg.tid, msg.addr, msg.ctx,
              localcnt,  msg.result, 0, sol_size, msg.bid, msg.sctx, false, 0, 0);
          solution_queue.push(rgd_sol);
        }
      } else if msg.msgtype == 3 {
        //offset
        add_cons(msg.label, &table, &ctx, &solver, &mut uf, &mut branch_deps);
      } else {
        //size
      }
      debug!("solving eplased {}", t_start.elapsed().as_secs());
      if t_start.elapsed().as_secs() > 90 { break; }
    } else {
      break;
    }
  }
  unsafe { libc::shmdt(rawptr) };
}
