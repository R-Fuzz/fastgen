use crate::rgd::*;
use crate::union_table::*;
use crate::union_to_ast::*;
//use crate::util::*;
use crate::analyzer::*;
use std::collections::HashMap;
use std::collections::HashSet;
use std::collections::VecDeque;
use std::sync::{RwLock,Arc};
use crate::cpp_interface::*;
use protobuf::Message;
use crate::union_find::*;
use std::time;

//each input offset has a coresspdoing slot
pub struct BranchDep {
  //the dependent expr labels associated with this input
  pub expr_labels: HashSet<u32>,
  // the dependent input offsets associated with this input offset
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

pub fn scan_nested_tasks(labels: &Vec<(u32,u32,u64,u64,u64,u32,u32)>, memcmp_data: &mut VecDeque<Vec<u8>>,
          table: &UnionTable, tainted_size: usize, branch_gencount: &Arc<RwLock<HashMap<(u64,u64,u32,u64), u32>>>
          , branch_hitcount: &Arc<RwLock<HashMap<(u64,u64,u32,u64), u32>>>, buf: &Vec<u8>) {
  let mut branch_deps: Vec<Option<BranchDep>> = Vec::with_capacity(tainted_size);
  let mut uf = UnionFind::new(tainted_size);
  branch_deps.resize_with(tainted_size, || None);
  //let mut cons_table = HashMap::new();
  //branch_deps.push(Some(BranchDep {expr_labels: HashSet::new(), input_deps: HashSet::new()}));
  let t_start = time::Instant::now();
  let mut count = 0;
  for &label in labels {
    let mut hitcount = 1;
    let mut gencount = 0;
    if branch_hitcount.read().unwrap().contains_key(&(label.3,label.4,label.5,label.2)) {
      hitcount = *branch_hitcount.read().unwrap().get(&(label.3,label.4,label.5,label.2)).unwrap();
      hitcount += 1;
    }
    branch_hitcount.write().unwrap().insert((label.3,label.4,label.5,label.2), hitcount);

    if branch_gencount.read().unwrap().contains_key(&(label.3,label.4,label.5,label.2)) {
      gencount = *branch_gencount.read().unwrap().get(&(label.3,label.4,label.5,label.2)).unwrap();
    }
    //we have to continue here, the underlying task lookup with check the redudant as well. If it is redudant, the task will be loaded
    //without inserting caching 

    if hitcount > 1 {
      if label.6 == 2 {
        memcmp_data.pop_front().unwrap();
        continue;
      }
    }


    let mut node = AstNode::new();
    let mut cons = Constraint::new();
    //let mut cons_reverse = Constraint::new();
    let mut inputs = HashSet::new();
    if label.6 == 1 {
      get_gep_constraint(label.1, label.2, &mut node, table, &mut inputs);
    } else if label.6 == 0 {
      get_one_constraint(label.1, label.2 as u32, &mut node, table, &mut inputs);
    } else if label.6 == 2 {
      let data = memcmp_data.pop_front().unwrap();
      let (index, size) = get_fmemcmp_constraint(label.2 as u32, table, &mut inputs);
      if data.len() >= size {
        unsafe { submit_fmemcmp(data.as_ptr(), index, size as u32, label.0, label.3); }
      }
      continue;
    } else if label.6 == 3 {
      get_addcons_constraint(label.1, label.2 as u32, &mut node, table, &mut inputs);
    }


    if inputs.is_empty() { 
	//warn!("Skip constraint!"); 
     continue; }

    let mut init = false;
    //build union table
    let mut v0 = 0;
    for &v in inputs.iter() {
      if !init {
        v0 = v;
        init = true;
      }
      uf.union(v as usize, v0 as usize);
    }
      //step 2: add constraints
      let mut added = HashSet::new();
      for off in uf.get_set(v0 as usize) {
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
/*
      let bytes = cons.write_to_bytes().unwrap();
      let mut stream = CodedInputStream::from_bytes(&bytes);
      stream.set_recursion_limit(1000);
      cons_reverse.merge_from(&mut stream).expect("merge failed");
*/
      let mut task = SearchTask::new();
      //cons_table.insert(label.1, cons.clone());
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

      let task_ser = task.write_to_bytes().unwrap();
/*
        count = count + 1;
      	unsafe { submit_task(task_ser.as_ptr(), task_ser.len() as u32, true); }
*/


      if hitcount <= 5 && gencount == 0 && label.6 !=3 {
      	unsafe { submit_task(task_ser.as_ptr(), task_ser.len() as u32, true); }
      } else {
      	unsafe { submit_task(task_ser.as_ptr(), task_ser.len() as u32, false); }
      }




      

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
            Some(BranchDep {expr_labels: HashSet::new()});
      }
      let deps_opt = &mut branch_deps[off as usize];
      let deps = deps_opt.as_mut().unwrap(); 
      deps.expr_labels.insert(label.1);
    }
  let used_t1 = t_start.elapsed().as_secs() as u32;
        if (used_t1 > 90)  {//1s
          break;
        }
  }
  info!("submitted {} tasks", count);
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


fn analyze_meta(cons: &mut Constraint, buf: &Vec<u8>) {
  let mut local_map = HashMap::new();
  let mut shape = HashMap::new();
  let mut input_args = Vec::new();
  let mut inputs = Vec::new();
  let mut visited = HashSet::new();
  let mut const_num = 0;
  map_args(cons.mut_node(), &mut local_map, &mut shape, &mut input_args, &mut inputs, &mut visited, &mut const_num, buf);
  append_meta(cons, &local_map, &shape, &input_args, &inputs, const_num);
}


#[cfg(test)]
mod tests {
  use super::*;
  use crate::cpp_interface::*;
  use protobuf::Message;
  use crate::fifo::*;
  use crate::util::*;
  use fastgen_common::config;
  use std::path::Path;
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
    let (labels,mut fmemcmpdata) = read_pipe(2);
    println!("labels len is {}", labels.len());
    let dedup = Arc::new(RwLock::new(HashSet::<(u64,u64,u32,u64)>::new()));
    let branch_hit = Arc::new(RwLock::new(HashMap::<(u64,u64,u32), u32>::new()));
    //let mut buf: Vec<u8> = Vec::with_capacity(15000);
    //buf.resize(15000, 0);
    let file_name = Path::new("/home/cju/fastgen/test/seed");
    let buf = read_from_file(&file_name);
    println!("before scanning\n");
    scan_nested_tasks(&labels, &mut fmemcmpdata, table, 15000, &dedup, &branch_hit, &buf);
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
