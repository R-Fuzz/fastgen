use crate::rgd::*;
use crate::util::*;
use std::collections::HashMap;
use std::collections::HashSet;
use std::path::Path;
use crate::gd::*;
use crate::task::Cons;
use crate::task::Fut;
use crate::jit::JITEngine;
use crate::solution::Solution;
use blockingqueue::BlockingQueue;
use std::time;
use inkwell::execution_engine::JitFunction;
use crate::jit::JigsawFnType;
use std::hash::{Hash, Hasher};
use crate::op_def::*;
use std::rc::Rc;
use std::cell::RefCell;
use crate::union_find::*;

#[derive(Clone)]
pub struct OneLabelCons<'a> {
  pub land_lor: bool, //true for land, false for lor
  pub members: Vec<Rc<RefCell<Cons<'a>>>>,
}

#[derive(Clone)]
pub struct BranchDep<'a> {
  pub cons_set: Vec<OneLabelCons<'a>>,
}


pub struct AstNodeWrapper(AstNode);

static mut gengine: Option<JITEngine> = None;
static mut gfuncache: Option<HashMap<AstNodeWrapper, JitFunction<JigsawFnType>>> = None;
//static mut branch_deps: Option<Vec<Option<BranchDep>>> = None;
static mut miss: u32 = 0;
static mut hit : u32 = 0;


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
          return eq_cur(&current.get_children()[i], &other.get_children()[i]);
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
          return eq_cur(&current.get_children()[i], &other.get_children()[i]);
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
          return eq_cur(&current.get_children()[i], &other.get_children()[i]);
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
    return eq_cur(&current.get_children()[i], &other.get_children()[i]);
  }

  true 
}

impl PartialEq for AstNodeWrapper {
  fn eq(&self, other: &Self) -> bool {
    return eq_cur(&self.0, &other.0); 
  }
}

impl Hash for AstNodeWrapper {
  fn hash<H: Hasher>(&self, state: &mut H) {
    self.0.get_hash().hash(state);
  }
}

/*
impl Eq for AstNode {}

impl Hash for AstNode {
  fn hash<H: Hasher>(&self, state: &mut H) {
    self.get_hash().hash(state);
  }
}
*/


pub struct SearchTaskBuilder<'a> {
  pub per_session_cache: HashMap<u32, Constraint>,  
  pub last_fid: u32,
  pub branch_deps: Vec<Option<BranchDep<'a>>>,
  pub uf:UnionFind,
}

impl<'a> SearchTaskBuilder<'a> {
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

  pub fn construct_task(&mut self, task: &SearchTask, solve: bool, inputs: &HashSet<u32>) -> Fut<'a> {
 // pub fn construct_task(& mut self, task: &SearchTask, engine: &JITEngine, 
  //    fun_cache: & mut HashMap<AstNodeWrapper, JitFunction<JigsawFnType>>,
   //   solve: bool, inputs: &HashSet<u32>) -> Fut {
    //pub fn construct_task(&mut self, task: &SearchTask, engine: &JITEngine) -> Fut {
    unsafe {
    if gengine.is_none() {
        gengine = Some(JITEngine::new());
    }
    if gfuncache.is_none() {
        gfuncache = Some(HashMap::new());
    }
    let engine = gengine.as_ref().unwrap();
    let func_cache = gfuncache.as_mut().unwrap();

    //Union 
    let mut init = false;
    let mut v0 = 0;
    for &v in inputs.iter() {
      if !init {
        v0 = v;
        init = true;
      }
      self.uf.union(v as usize, v0 as usize);
    }

    let mut fut = Fut::new();
    let mut onecons = OneLabelCons{land_lor:false, members:Vec::new()};
    for constraint in task.get_constraints() {
      let mut cons = Rc::new(RefCell::new(Cons::new()));
      self.append_meta(&cons, &constraint);
      if !func_cache.contains_key(&AstNodeWrapper(constraint.get_node().clone())) {
        let fun = engine.add_function(&constraint.get_node(), &cons.borrow().local_map);
        //println!("miss and jitime is {}", t_start.elapsed().as_micros());
        unsafe { println!("miss/hit {}/{}", miss,hit); }
        unsafe { miss += 1; }
        func_cache.insert(AstNodeWrapper(constraint.get_node().clone()), fun.clone());
        cons.borrow_mut().set_func(fun);
      } else {
        let fun = func_cache[&AstNodeWrapper(constraint.get_node().clone())].clone();
        cons.borrow_mut().set_func(fun);
        unsafe { hit += 1; }
        //println!("hit and jitime is {}", t_start.elapsed().as_micros());
      }

      fut.constraints.push(cons.clone());
      onecons.members.push(cons.clone());
    }

    //add nested constraint  
    for off in self.uf.get_set(v0 as usize) {
      let deps_opt = &self.branch_deps[off as usize];
      if let Some(deps) = deps_opt {
        for onelabel in &deps.cons_set {
            for onecons in &onelabel.members {
              fut.constraints.push(onecons.clone());
            }
        }
      }
    }

    //add to nested dependency tree
    for &off in inputs.iter() {
      let mut is_empty = false;
      {
        let deps_opt = &self.branch_deps[off as usize];
        if deps_opt.is_none() {
          is_empty = true;
        }
      }
      if is_empty {
        self.branch_deps[off as usize] = Some(BranchDep {cons_set: Vec::new()});
      }
      let deps_opt = &mut self.branch_deps[off as usize];
      let deps = deps_opt.as_mut().unwrap();
      deps.cons_set.push(onecons);
      break;
    }

    if solve {
      fut.finalize();
    }
    fut
    }
  }

  pub fn append_meta(&self, cons: &Rc<RefCell<Cons>>, constraint: &Constraint) {
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
    cons.borrow_mut().comparison = constraint.get_node().get_kind();
    cons.borrow_mut().const_num = constraint.get_meta().get_const_num();
  }

  //submit a sinlge branch
  pub fn submit_task_rust(&mut self, task: &SearchTask, 
      solution_queue: BlockingQueue<Solution>,
      solve: bool, inputs: &HashSet<u32>) {

    /*
       info!("print task number of children is {} fid {}",task.get_constraints().len(), task.get_fid());
       print_task(task);
       let r = save_request(task, &Path::new("saved_test"));
       if r.is_err() {
       debug!("save error");
       }
     */    
      let mut fut = self.construct_task(task, solve, inputs);
      if solve {
      gd_search(&mut fut);
      for sol in fut.rgd_solutions {
        let sol_size = sol.len();
        let rgd_sol = Solution::new(sol, task.get_fid(), task.get_addr(), task.get_ctx(), 
            task.get_order(), task.get_direction(), 0, sol_size);
        solution_queue.push(rgd_sol);
      }
      }
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
