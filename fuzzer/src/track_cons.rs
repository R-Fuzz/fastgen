use crate::rgd::*;
use crate::union_table::*;
use crate::union_to_ast::*;
//use crate::util::*;
use crate::analyzer::*;
use std::collections::HashMap;
use std::collections::HashSet;


pub fn scan_tasks(labels: &Vec<(u32,u32,u32)>, tasks: &mut Vec<SearchTask>, table: &UnionTable) {
  for &label in labels {
    let mut node = AstNode::new();
    let mut cons = Constraint::new();
    get_one_constraint(label.1, label.2, &mut node, table);
    cons.set_node(node);
    analyze_meta(&mut cons);
    let mut task = SearchTask::new();
    task.mut_constraints().push(cons);
    task.set_fid(label.0);
    tasks.push(task);
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


fn analyze_meta(cons: &mut Constraint) {
  let mut local_map = HashMap::new();
  let mut input_args = Vec::new();
  let mut inputs = Vec::new();
  let mut visited = HashSet::new();
  let mut const_num = 0;
  map_args(cons.mut_node(), &mut local_map, &mut input_args, &mut inputs, &mut visited, &mut const_num);
  append_meta(cons, &local_map, &input_args, &inputs, const_num);
}


#[cfg(test)]
mod tests {
  use super::*;
  use crate::cpp_interface::*;
  use protobuf::Message;
  use crate::fifo::*;
  //use crate::util::*;

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

    let mut tasks = Vec::new();
    let labels = read_pipe();
    println!("labels len is {}", labels.len());
    scan_tasks(&labels, &mut tasks, table); 
    unsafe { init_core(false,true); }
    for task in tasks {
      //print_task(&task);
      let task_ser = task.write_to_bytes().unwrap();
      unsafe { submit_task(task_ser.as_ptr(), task_ser.len() as u32); }
    }
    unsafe { aggregate_results(); }
    unsafe { fini_core(); }
  }
}
