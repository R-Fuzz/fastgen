use crate::rgd::*;
use crate::union_table::*;
use crate::union_to_ast::*;
use crate::util::*;



pub fn scan_tasks(tasks: &mut Vec<SearchTask>, table: &UnionTable) {
  let labels = vec![42,44];
  for label in labels {
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
