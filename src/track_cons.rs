use crate::rgd::*;
use crate::union_table::*;
use crate::union_to_ast::*;
use crate::util::*;

pub fn scan_tasks(labels: &Vec<u32>, tasks: &mut Vec<SearchTask>, table: &UnionTable) {
  for &label in labels {
    let mut real_left = RealAstNode::new();
    let mut real_right = RealAstNode::new();
    let mut cons = Constraint::new();
    let op = get_one_constraint(label, &mut real_left, &mut real_right, table);
    let mut left = AstNode::new();
    let mut right = AstNode::new();
    left.set_payload(real_left);
    right.set_payload(real_right);
    cons.set_left(left);
    cons.set_right(right);
    cons.set_comparison(to_rgd_op(op));
    let mut task = SearchTask::new();
    task.mut_constraints().push(cons);
    tasks.push(task);
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::cpp_interface::*;
  use protobuf::Message;

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
    let labels = vec![43,44];
    scan_tasks(&labels, &mut tasks, table); 
    for task in tasks {
      let task_ser = task.write_to_bytes().unwrap();
      unsafe { print_buffer(task_ser.as_ptr(), task_ser.len() as u32); }
    }
  }
}
