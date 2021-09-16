use crate::rgd::*;
use crate::util::*;
use std::collections::HashMap;
use std::collections::HashSet;
use std::path::Path;
use crate::gd::*;
use crate::task::Cons;
use crate::task::Fut;
use crate::jit::JITEngine;
use crate::solution::Solution; use blockingqueue::BlockingQueue;
use std::time;
use inkwell::execution_engine::JitFunction;
use crate::jit::JigsawFnType;
use std::hash::{Hash, Hasher};
use crate::op_def::*;
use std::rc::Rc;
use std::cell::RefCell;
use crate::union_find::*;
use crate::search_task::SearchTask;
use crate::analyzer::*;
use protobuf::Message;
use crate::cpp_interface;
use protobuf::CodedInputStream;

#[derive(Clone)]
pub struct OneLabelCons {
  pub members: Vec<Vec<Rc<RefCell<Cons>>>>,
}

#[derive(Clone)]
pub struct BranchDep {
  pub cons_set: Vec<OneLabelCons>,
}


pub struct AstNodeWrapper(Vec<u8>);

pub static mut GENGINE: Option<JITEngine> = None;
//AstNode -> function address
pub static mut GFUNCACHE: Option<HashMap<AstNodeWrapper, usize>> = None;
pub static mut MISS: u32 = 0;
pub static mut HIT: u32 = 0;
//
pub static mut JTIME: u32 = 0;
pub static mut STIME: u32 = 0;


impl Eq for AstNodeWrapper {
}

fn eq_cur(current: &AstNode, other: &AstNode) -> bool {

  if current.get_kind() >= RGD::Ult as u32 && 
    current.get_kind() <= RGD::Uge as u32 &&
      other.get_kind() <= RGD::Uge as u32 &&
      other.get_kind() >= RGD::Ult as u32  {
        let children_size = current.get_children().len();
        if current.get_children().len() != other.get_children().len() {
          return false;
        }
        for i in 0..children_size  {
          if !eq_cur(&current.get_children()[i], &other.get_children()[i]) {
            return false;
          }
        }
        return true;
  }

  if current.get_kind() >= RGD::Slt as u32 && 
    current.get_kind() <= RGD::Sge as u32 &&
      other.get_kind() <= RGD::Sge as u32 &&
      other.get_kind() >= RGD::Slt as u32  {
        let children_size = current.get_children().len();
        if current.get_children().len() != other.get_children().len() {
          return false;
        }
        for i in 0..children_size  {
          if !eq_cur(&current.get_children()[i], &other.get_children()[i]) {
            return false;
          }
        }
        return true;
  }

  if current.get_kind() >= RGD::Equal as u32 && 
    current.get_kind() <= RGD::Distinct as u32 &&
      other.get_kind() >= RGD::Equal as u32 &&
      other.get_kind() <= RGD::Distinct as u32  {
        let children_size = current.get_children().len();
        if current.get_children().len() != other.get_children().len() {
          return false;
        }
        for i in 0..children_size  {
          if !eq_cur(&current.get_children()[i], &other.get_children()[i]) {
            return false;
          }
        }
        return true;
  }
  
  if current.get_hash() != other.get_hash() { return false; }
  if current.get_kind() != other.get_kind() { return false; }
  if current.get_bits() != other.get_bits() { return false; }
  let children_size = current.get_children().len();
  if current.get_children().len() != other.get_children().len() {
    return false;
  }
  for i in 0..children_size  {
    if !eq_cur(&current.get_children()[i], &other.get_children()[i]) {
      return false;
    }
  }

  true 
}

impl PartialEq for AstNodeWrapper {
  fn eq(&self, other: &Self) -> bool {

    let mut stream_this = CodedInputStream::from_bytes(&self.0);
    stream_this.set_recursion_limit(1000);
    let mut this_node = AstNode::new();
    this_node.merge_from(&mut stream_this);

    let mut stream_other = CodedInputStream::from_bytes(&other.0);
    stream_other.set_recursion_limit(1000);
    let mut other_node = AstNode::new();
    other_node.merge_from(&mut stream_other);
    return eq_cur(&this_node, &other_node); 
  }
}

impl Hash for AstNodeWrapper {
  fn hash<H: Hasher>(&self, state: &mut H) {
    let mut stream_this = CodedInputStream::from_bytes(&self.0);
    stream_this.set_recursion_limit(1000);
    let mut this_node = AstNode::new();
    this_node.merge_from(&mut stream_this);
    this_node.get_hash().hash(state);
  }
}


pub struct SearchTaskBuilder {
  pub per_session_cache: HashMap<u32, Constraint>,  
  pub last_fid: u32,
  pub branch_deps: Vec<Option<BranchDep>>,
  pub uf:UnionFind,
}


pub fn init_engine() {
    unsafe {
      if GENGINE.is_none() {
        GENGINE = Some(JITEngine::new());
        GENGINE.as_mut().unwrap().init();
      }
      if GFUNCACHE.is_none() {
        GFUNCACHE = Some(HashMap::new());
      }
    }
  }

impl SearchTaskBuilder {
  pub fn new(tainted_size: usize) -> Self {
    unsafe {
    Self {
      per_session_cache: HashMap::new(),
      last_fid: std::u32::MAX,
      uf: UnionFind::new(tainted_size),
      branch_deps: vec![None;tainted_size],
    }
    }
  }

  fn union(&mut self, inputs: &HashSet<u32>) -> u32 {
    //UnionFind union
    let mut init = false;
    let mut v0 = 0;
    for &v in inputs.iter() {
      if !init {
        v0 = v;
        init = true;
      }
      self.uf.union(v as usize, v0 as usize);
    }
    v0
  }

  

  fn task_jit(&self, target_cons: &(Vec<Vec<Rc<Constraint>>>, bool)) -> Vec<Vec<Rc<RefCell<Cons>>>> {
    unsafe {
      let mut engine = GENGINE.as_mut().unwrap();
      let func_cache = GFUNCACHE.as_mut().unwrap();

      //build cons set
      let mut cons_set = Vec::new();
      //for each land
      for land in &target_cons.0 {
        let mut row = Vec::new();
        for constraint in land {
          let mut cons = Rc::new(RefCell::new(Cons::new()));
          if !self.append_meta(&cons, &constraint, target_cons.1) {
            continue;
          }
         // if !func_cache.contains_key(&AstNodeWrapper(constraint.get_node().clone())) {
         // if !cpp_interface::contains(node_ser.as_ptr(), node_ser.len()) {
          if !func_cache.contains_key(&AstNodeWrapper(constraint.get_node().write_to_bytes().unwrap())) {
            let t_start = time::Instant::now();
            let fun = engine.add_function(&constraint.get_node(), &cons.borrow().local_map);
            if fun.is_some() {
              let jtime = t_start.elapsed().as_micros();
              
              let fun_extract = fun.unwrap();
              unsafe { JTIME += jtime as u32; }
              unsafe { info!("miss/hit {}/{}, jitime {} totoal {}", MISS, HIT, jtime, JTIME); }
              unsafe { MISS += 1; }
              func_cache.insert(AstNodeWrapper(constraint.get_node().write_to_bytes().unwrap()), fun_extract);
        //      cpp_interface::add(node_ser.as_ptr(), node_ser.len(), idx);
              cons.borrow_mut().set_func(fun_extract);
            } else {
              continue;
            }
          } else {
            let fun_idx = func_cache[&AstNodeWrapper(constraint.get_node().write_to_bytes().unwrap())];
            //let fun_idx = cpp_interface::get(node_ser.as_ptr(),node_ser.len());
            cons.borrow_mut().set_func(fun_idx);
            unsafe { HIT += 1; }
            //info!("hit and jitime is {}", t_start.elapsed().as_micros());
          }
          row.push(cons.clone());
        }
        cons_set.push(row);
      }
      cons_set
    }
  }


  pub fn add_dependency(&mut self, task: &SearchTask, inputs: &HashSet<u32>, v0: u32) {
    //jit the function of the task
    let all_branch_cons = self.task_jit(&task.path_cons);
    let mut onecons = OneLabelCons{members: all_branch_cons};


    //add to nested dependency tree
    let mut is_empty = false;
    {
      let deps_opt = &self.branch_deps[v0 as usize];
      if deps_opt.is_none() {
        is_empty = true;
      }
    }
    if is_empty {
      self.branch_deps[v0 as usize] = Some(BranchDep {cons_set: Vec::new()});
    }
    let deps_opt = &mut self.branch_deps[v0 as usize];
    let deps = deps_opt.as_mut().unwrap();
    deps.cons_set.push(onecons);
  }

  pub fn break_disjoint(&mut self, land: &Vec<Rc<RefCell<Cons>>>) -> Vec<Fut> {
    let mut res = Vec::new();
    let mut global_map = HashMap::new();
    for item in land {
      let mut added = false;
      let mut cur_idx = 0;
      for &k in item.borrow().local_map.keys() {
        if !global_map.contains_key(&k) {
          if !added {
            let mut fut = Fut::new();
            fut.constraints.push(item.clone());
            cur_idx = res.len();
            added = true;
            global_map.insert(k,  cur_idx);
            res.push(fut);
          } else {
            global_map.insert(k, cur_idx);
          }
        } else {
          if !added {
            let cur_idx = global_map[&k];
            res[cur_idx].constraints.push(item.clone());
            added = true;
          }
        }
      }
    }
    res
  }

  //vector of disjointed futs
  pub fn construct_task(&mut self, task: &SearchTask, inputs: &HashSet<u32>, v0: u32) -> (Vec<Vec<Fut>>, Vec<Vec<Fut>>) {

    //jit the function of the task
    let mut all_branch_cons = self.task_jit(&task.flip_cons);

    //cross-product of the dependecies
    let mut res_opt = Vec::new();
    for land in &all_branch_cons {
      let disjoint = self.break_disjoint(&land);
      res_opt.push(disjoint);
    }

/*
    for off in self.uf.get_set(v0 as usize) {
      let deps_opt = &self.branch_deps[off as usize];
      if let Some(deps) = deps_opt {
        for onelabel in &deps.cons_set {
          let cur = all_branch_cons;
          all_branch_cons = Vec::new();
          for row in cur {
            for row1 in &onelabel.members {
              let mut new_row = row.clone();
              new_row.extend(row1.clone()); 
              all_branch_cons.push(new_row);
            }
          }
        }
      }
    }

*/
    for row in &mut all_branch_cons {
      for off in self.uf.get_set(v0 as usize) {
        let deps_opt = &self.branch_deps[off as usize];
        if let Some(deps) = deps_opt {
          for onelabel in &deps.cons_set {
              if onelabel.members.len() > 0 {
                row.extend(onelabel.members[0].clone());
              }
          }
        }
      }
    }


    let mut res_nes = Vec::new();
    for land in &all_branch_cons {
      let disjoint = self.break_disjoint(&land);
      res_nes.push(disjoint);
    }
    (res_nes, res_opt)
  }

  pub fn append_meta(&self, cons: &Rc<RefCell<Cons>>, 
                      constraint: &Constraint, flip: bool) -> bool {
    debug!("append_meta flip {}", flip);
    //print_node(constraint.get_node());
    for amap in constraint.get_meta().get_map() {
      cons.borrow_mut().local_map.insert(amap.get_k(), amap.get_v());
    }
    for aarg in constraint.get_meta().get_args() {
      cons.borrow_mut().input_args.push((aarg.get_isinput(), aarg.get_v()));
    }
    for ainput in constraint.get_meta().get_inputs() {
      cons.borrow_mut().inputs.insert(ainput.get_offset(), ainput.get_iv() as u8);
    }
    for ashape in constraint.get_meta().get_shape() {
      cons.borrow_mut().shape.insert(ashape.get_offset(), ashape.get_start());
    }
    let mut comp = constraint.get_node().get_kind();
    if (flip) { comp = get_flipped_op(comp); }
    if comp == 0 {
      return false;
    }
    cons.borrow_mut().comparison = comp;
    cons.borrow_mut().const_num = constraint.get_meta().get_const_num();
    return true;
  }

  //submit a sinlge branch
  pub fn submit_task_rust(&mut self, task: &SearchTask, 
      solution_queue: BlockingQueue<Solution>,
      solve: bool, inputs: &HashSet<u32>) {
    /*
       let r = save_request(task, &Path::new("saved_test"));
       if r.is_err() {
       debug!("save error");
       }
     */    

    //union table build
    let v0 = self.union(inputs);   

    let mut res = self.construct_task(task, inputs, v0);

    let mut opt_solved = false;
    let mut nest_solved = false;

    if solve {
      for mut disjoints in res.1 {
        let mut result = true;
        let mut overall_sol = HashMap::new();
        for mut fut in disjoints {
          fut.finalize();
          result = result && gd_search(&mut fut);
          debug!("opt search result {}", result);
          if !result { break; }
          for sol in fut.rgd_solutions {
            for (k,v) in sol.iter() {
              trace!("k: {} v: {}",k,v);
              overall_sol.insert(*k,*v);
            }
          }

        }
        let sol_size = overall_sol.len();
        if result {
          let rgd_sol = Solution::new(overall_sol, task.fid, task.addr, task.ctx, 
              task.order, task.direction, 0, sol_size, task.bid, task.sctx);
          solution_queue.push(rgd_sol);
        }
        if result { opt_solved = true; break; }
      }
    }

    if solve && opt_solved {
      let mut sub_clause_tried = 0;
      for mut disjoints in &mut res.0 {
        let mut result = true;
        let mut overall_sol = HashMap::new();
        for mut fut in disjoints {
          fut.finalize();
          result = result && gd_search(&mut fut);
          trace!("search result {}", result);
          if !result { break; }
          for sol in &fut.rgd_solutions {
            for (k,v) in sol.iter() {
              trace!("k: {} v: {}",k,v);
              overall_sol.insert(*k,*v);
            }
          }

        }
        let sol_size = overall_sol.len();
        if result {
          let rgd_sol = Solution::new(overall_sol, task.fid, task.addr, task.ctx, 
              task.order, task.direction, 0, sol_size, task.bid, task.sctx);
          solution_queue.push(rgd_sol);
        }
        sub_clause_tried += 1;
        if result { nest_solved = true; break; }
      }
    }

    self.add_dependency(task, inputs, v0);
  }
}

#[cfg(test)]
  mod tests {
    use crate::rgd::*;
    use crate::util::*;
    use crate::parser::*;
    use std::path::Path;
    use crate::gd::*;
    use crate::task::SContext;
    use std::collections::HashMap;
#[test]
    fn test_load() {
      let tasks: Vec<SearchTask> = load_request(Path::new("saved_test")).expect("ok");
      let mut tb = SearchTaskBuilder::new();
      let engine = JITEngine::new();
      let mut funcache = HashMap::new();
      for task in tasks { let task_copy = task.clone();
        print_task(&task_copy);
        let mut fut = tb.construct_task(&task_copy, &engine, &mut funcache);
        println!("search!");
        gd_search(&mut fut);
        for sol in fut.rgd_solutions {
          for (k,v) in sol.iter() {
            println!("k {} v {}", k, v);
          }
        }
      }
    }
#[test]  
    fn test_input() {
      let mut ctx = SContext::new(2,2,4);
      ctx.min_input.value.push(1);
      let mut input = &mut ctx.min_input;
      let mut scratch_input = input.clone();
      scratch_input.set(0,2);
      *input = scratch_input;
      println!("{}",ctx.min_input.get(0));
    }
  }
