use crate::rgd::*;
use crate::union_table::*;
use crate::union_to_ast::*;
//use crate::util::*;
use crate::analyzer::*;
use std::collections::HashMap;
use std::collections::HashSet;
use std::sync::{RwLock,Arc};
use crate::cpp_interface::*;
use protobuf::Message;

//each input offset has a coresspdoing slot
pub struct BranchDep {
  //the dependent expr labels associated with this input
  pub expr_labels: HashSet<u32>,
  // the dependent input offsets associated with this input offset
  pub input_deps: HashSet<u32>, 
}

//label 0: fid  label 1: label  label 2: direction/result label 3: addr, label4: ctx, label5, order label6: is gep
pub fn scan_tasks(labels: &Vec<(u32,u32,u64,u64,u64,u32,u32)>,
                  tasks: &mut Vec<SearchTask>,
                  table: &UnionTable,
                  buf: &Vec<u8>) {
  for &label in labels {
    let mut node = AstNode::new();
    let mut cons = Constraint::new();
    let mut deps = HashSet::new();
    get_one_constraint(label.1, label.2 as u32, &mut node, table, &mut deps);
    cons.set_node(node);
    analyze_meta(&mut cons, buf);
    let mut task = SearchTask::new();
    task.mut_constraints().push(cons);
    task.set_fid(label.0);
    task.set_addr(label.3);
    task.set_ctx(label.4);
    task.set_order(label.5);
    tasks.push(task);
  }
}

pub fn scan_nested_tasks(labels: &Vec<(u32,u32,u64,u64,u64,u32,u32)>, 
          table: &UnionTable, tainted_size: usize, dedup: &Arc<RwLock<HashSet<(u64,u64,u32, u64)>>>
          , branch_hitcount: &Arc<RwLock<HashMap<(u64,u64,u32), u32>>>, buf: &Vec<u8>) {
  let mut branch_deps: Vec<Option<BranchDep>> = Vec::with_capacity(tainted_size);
  branch_deps.resize_with(tainted_size, || None);
  //let mut cons_table = HashMap::new();
  //branch_deps.push(Some(BranchDep {expr_labels: HashSet::new(), input_deps: HashSet::new()}));
  let mut nbranches = 0;
  for &label in labels {
    let mut count = 1;
    if branch_hitcount.read().unwrap().contains_key(&(label.3,label.4,label.5)) {
      count = *branch_hitcount.read().unwrap().get(&(label.3,label.4,label.5)).unwrap();
      count += 1;
    }
    branch_hitcount.write().unwrap().insert((label.3,label.4,label.5), count);

    if dedup.read().unwrap().contains(&(label.3,label.4,label.5, label.2)) {
      continue;
    }
    dedup.write().unwrap().insert((label.3,label.4,label.5, label.2));
    let mut node = AstNode::new();
    let mut cons = Constraint::new();
    let mut inputs = HashSet::new();
    if label.6 == 1 {
      get_gep_constraint(label.1, label.2, &mut node, table, &mut inputs);
    } else if label.6 == 0 {
      get_one_constraint(label.1, label.2 as u32, &mut node, table, &mut inputs);
    } else if label.6 == 2 {
      unsafe { submit_fmemcmp(label.2, label.3, label.4); }
      continue;
    } else if label.6 == 3 {
     // get_addcons_constraint(label.1, label.2 as u32, &mut node, table, &mut inputs);
    }


    if inputs.is_empty() { warn!("Skip constraint!"); continue; }

    //Step 1: collect additional input deps
    let mut work_list = Vec::new();
    for &v in inputs.iter() {
      work_list.push(v);
    }
    while !work_list.is_empty() {
      let off = work_list.pop().unwrap();
      let deps_opt = &branch_deps[off as usize];
      if let Some(deps) = deps_opt {
        for &v in deps.input_deps.iter() {
          if inputs.insert(v) {
            work_list.push(v);
          }
        }
      }
    }

      //step 2: add constraints
      let mut added = HashSet::new();
      for &off in inputs.iter() {
        let deps_opt = &branch_deps[off as usize];
        if let Some(deps) = deps_opt {
          for &l in deps.expr_labels.iter() {
            added.insert(l);
          }
        }
      }

      //we dont solve add_cons
      // add constraints
      cons.set_node(node);
      analyze_meta(&mut cons, buf);
      cons.set_label(label.1);
      let mut task = SearchTask::new();
      task.mut_constraints().push(cons);
      for &l in added.iter() {
        //let mut c = cons_table[l].clone();
        //flip_op(c.mut_node());
        let mut c = Constraint::new();
        c.set_label(l);
        task.mut_constraints().push(c);
      }
      task.set_fid(label.0);
      task.set_addr(label.3);
      task.set_ctx(label.4);
      task.set_order(label.5);
      task.set_direction(label.2);
      //tasks.push(task);

      let task_ser = task.write_to_bytes().unwrap();
      unsafe { submit_task(task_ser.as_ptr(), task_ser.len() as u32, true); }

    //step 3: nested branch
    for &off in inputs.iter() {
      let mut is_empty = false;
      {
        let deps_opt = &branch_deps[off as usize];
        if deps_opt.is_none() {
          is_empty = true;
        }
      }
      if is_empty {
        branch_deps[off as usize] =
            Some(BranchDep {expr_labels: HashSet::new(), input_deps: HashSet::new()});
      }
      let deps_opt = &mut branch_deps[off as usize];
      let deps = deps_opt.as_mut().unwrap(); 
      for &off1 in inputs.iter() {
        deps.input_deps.insert(off1);
      }
      deps.expr_labels.insert(label.1);
    }
  }
}

fn append_meta(cons: &mut Constraint, 
              local_map: &HashMap<u32,u32>, 
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


fn analyze_meta(cons: &mut Constraint, buf: &Vec<u8>) {
  let mut local_map = HashMap::new();
  let mut input_args = Vec::new();
  let mut inputs = Vec::new();
  let mut visited = HashSet::new();
  let mut const_num = 0;
  map_args(cons.mut_node(), &mut local_map, &mut input_args, &mut inputs, &mut visited, &mut const_num, buf);
  append_meta(cons, &local_map, &input_args, &inputs, const_num);
}


#[cfg(test)]
mod tests {
  use super::*;
  use crate::cpp_interface::*;
  use protobuf::Message;
  use crate::fifo::*;
  use crate::util::*;
  use fastgen_common::config;
#[test]
  fn test_scan() {
    let id = unsafe {
      libc::shmget(
          0x1234,
          0xc00000000, 
          0o644 | libc::IPC_CREAT | libc::SHM_NORESERVE
          )
    };
    let ptr = unsafe { libc::shmat(id, std::ptr::null(), 0) as *mut UnionTable};
    let table = unsafe { & *ptr };

    unsafe { init_core(true,true); }
    let labels = read_pipe();
    println!("labels len is {}", labels.len());
    let dedup = Arc::new(RwLock::new(HashSet::<(u64,u64,u32,u64)>::new()));
    let branch_hit = Arc::new(RwLock::new(HashMap::<(u64,u64,u32), u32>::new()));
    let mut buf: Vec<u8> = Vec::with_capacity(15000);
    buf.resize(15000, 0);
    println!("before scanning\n");
    scan_nested_tasks(&labels, table, 15000, &dedup, &branch_hit, &buf);
    println!("after scanning\n");
//    scan_tasks(&labels, &mut tasks, table);
/*
    for task in tasks {
      println!("print task addr {} order {} ctx {}", task.get_addr(), task.get_order(), task.get_ctx());
      print_task(&task);
      let task_ser = task.write_to_bytes().unwrap();
      unsafe { submit_task(task_ser.as_ptr(), task_ser.len() as u32, true); }
    }
*/
    unsafe { aggregate_results(); }
    unsafe { fini_core(); }
  }
}
