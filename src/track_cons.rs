use crate::rgd::*;
use crate::union_table::*;
use crate::union_to_ast::*;
use crate::util::*;
use crate::analyzer::*;
use std::collections::HashMap;
use std::collections::HashSet;


pub fn scan_tasks(labels: &Vec<u32>, tasks: &mut Vec<SearchTask>, table: &UnionTable) {
  for &label in labels {
    let mut left = AstNode::new();
    let mut right = AstNode::new();
    let mut cons = Constraint::new();
    let op = get_one_constraint(label, &mut left, &mut right, table);
    cons.set_left(left);
    cons.set_right(right);
    cons.set_comparison(to_rgd_op(op));
    analyze_meta(&mut cons);
    let mut task = SearchTask::new();
    task.mut_constraints().push(cons);
    tasks.push(task);
  }
}

fn analyze_meta_one(node: &mut AstNode) {
  print_node(node);
  let mut local_map = HashMap::new();
  let mut input_args = Vec::new();
  let mut inputs = Vec::new();
  let mut visited = HashSet::new();
  let mut const_num = 0;
  map_args(node, &mut local_map, &mut input_args, &mut inputs, &mut visited, &mut const_num);
  println!("local map {:?}", local_map);
  println!("inputs {:?}", inputs);
  println!("input args {:?}", input_args);
}

fn analyze_meta(cons: &mut Constraint) {
  analyze_meta_one(cons.mut_left());
  analyze_meta_one(cons.mut_right());
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::cpp_interface::*;
  use protobuf::Message;
  use crate::fifo::*;

#[test]
  fn test_scan() {
    let id = unsafe {
      libc::shmget(
          0x1234,
          0xc00000000, 
          0644 | libc::IPC_CREAT | libc::SHM_NORESERVE
          )
    };
    let ptr = unsafe { libc::shmat(id, std::ptr::null(), 0) as *mut UnionTable};
    let table = unsafe { & *ptr };

    let mut tasks = Vec::new();
    let labels = read_pipe();
    scan_tasks(&labels, &mut tasks, table); 
    for task in tasks {
      print_task(&task);
      let task_ser = task.write_to_bytes().unwrap();
      unsafe { print_buffer(task_ser.as_ptr(), task_ser.len() as u32); }
    }
  }
}
