use crate::rgd::*;
use crate::union_table::*;
use crate::union_to_ast::*;
use crate::util::*;
use crate::analyzer::*;
use std::collections::HashMap;
use std::collections::HashSet;
use std::collections::VecDeque;
use std::sync::{RwLock,Arc};
use crate::cpp_interface::*;
use protobuf::Message;
use crate::union_find::*;
use std::time;
use crate::parser::*;
use blockingqueue::BlockingQueue;
use crate::solution::Solution;
use crate::search_task::SearchTask;
use crate::op_def::*;
use z3::{Solver, Config, Context, ast, Model};
use z3::ast::Ast;
use crate::z3solver::*;
use fastgen_common::config;

pub fn scan_nested_tasks(labels: &Vec<(u32,u32,u64,u64,u64,u32,u32,u32,u32)>, memcmp_data: &mut VecDeque<Vec<u8>>,
    table: &UnionTable, tainted_size: usize, 
    branch_gencount: &Arc<RwLock<HashMap<(u64,u64,u32,u64), u32>>>,
    branch_fliplist: &Arc<RwLock<HashSet<(u64,u64,u32,u64)>>>,
    branch_hitcount: &Arc<RwLock<HashMap<(u64,u64,u32,u64), u32>>>, buf: &Vec<u8>,
    tb: &mut SearchTaskBuilder, solution_queue: BlockingQueue<Solution>) {


  let mut cfg = Config::new();
  cfg.set_timeout_msec(10000);
  let ctx = Context::new(&cfg);
  let solver = Solver::new(&ctx);

  let t_start = time::Instant::now();
  let mut count = 0;
  let mut branch_local = HashMap::<(u64,u64),u32>::new();
  for &label in labels {
    let mut hitcount = 1;
    let mut gencount = 0;
    let mut flipped = false;
    let mut localcnt = 1;

    if branch_local.contains_key(&(label.3,label.4)) {
      localcnt = *branch_local.get(&(label.3,label.4)).unwrap();
      localcnt += 1;
    }
    branch_local.insert((label.3,label.4), localcnt);


    if localcnt > 64 {
      //pop data for fmemecmp 
      if label.6 == 2 {
        memcmp_data.pop_front().unwrap();
      }
      continue;
    }

    if branch_hitcount.read().unwrap().contains_key(&(label.3,label.4,localcnt,label.2)) {
      hitcount = *branch_hitcount.read().unwrap().get(&(label.3,label.4,localcnt,label.2)).unwrap();
      hitcount += 1;
    }
    branch_hitcount.write().unwrap().insert((label.3,label.4,localcnt,label.2), hitcount);

    if branch_fliplist.read().unwrap().contains(&(label.3,label.4,localcnt,label.2)) {
      //info!("the branch is flipped");
      flipped = true;
    }

    if branch_gencount.read().unwrap().contains_key(&(label.3,label.4,localcnt,label.2)) {
      gencount = *branch_gencount.read().unwrap().get(&(label.3,label.4,localcnt,label.2)).unwrap();
    }

    let mut node_opt: Option<AstNode> = None;
    //let mut cons_reverse = Constraint::new();
    let mut inputs = HashSet::new();
    let mut node_cache = HashMap::new();
    if label.6 == 1 {
      node_opt = get_gep_constraint(label.1, label.2, table, &mut inputs, &mut node_cache);
    } else if label.6 == 0 {
      node_opt = get_one_constraint(label.1, label.2 as u32, table, &mut inputs, &mut node_cache);
    } else if label.6 == 2 {
      let data = memcmp_data.pop_front().unwrap();
      let (index, size) = get_fmemcmp_constraint(label.1 as u32, table, &mut inputs);
      if data.len() >= size {
        //unsafe { submit_fmemcmp(data.as_ptr(), index, size as u32, label.0, label.3); }
        let mut sol = HashMap::new(); 
        for i in 0..data.len() - 1 {  //minus 1
          sol.insert(index + i as u32, data[i]);
        }
        let rsol = Solution::new(sol, label.0, label.3, 0, 0, 0, index as usize, size, 0, 0);
        solution_queue.push(rsol);
      }
      continue;
    } else if label.6 == 3 {
      node_opt = get_addcons_constraint(label.1, label.2 as u32, table, &mut inputs, &mut node_cache);
    }


    if let Some(node) = node_opt { 
      //print_node(&node);

      debug!("direction is {}",label.2);

      let breakdown = to_dnf(&node);
      let cons_breakdown = analyze_maps(&breakdown, &node_cache, buf);
      let reverse_cons_breakdown = de_morgan(&cons_breakdown);
      //cons_breakdown is a lor of lands
/*
      for row in &cons_breakdown {
        for item in row {
          print_node(&item.get_node());
        }
      }
*/
      let mut task;
      if label.2 == 1 {
        task = SearchTask::new((reverse_cons_breakdown,true), 
                              (cons_breakdown,false), 
                              label.0, label.3, label.4, localcnt, label.2, label.7, label.8);
      } else {
        task = SearchTask::new((cons_breakdown, false), 
                            (reverse_cons_breakdown, true), 
                            label.0, label.3, label.4, localcnt, label.2, label.7, label.8);
      }

      //tb.submit_task_rust(&task, solution_queue.clone(), true, &inputs);
     
      if hitcount <= 5 && (!flipped) && label.6 != 3 && localcnt <= 16 {
        if !tb.submit_task_rust(&task, solution_queue.clone(), true, &inputs) {
          if label.6 == 0 && config::HYBRID_SOLVER {
            if let Some(sol) =  solve_cond(label.1, label.2, table, &ctx, &solver) {
              let sol_size = sol.len();
              let z3_sol = Solution::new(sol, task.fid, task.addr, 
                  task.ctx, task.order, task.direction, 0, sol_size, task.bid, task.sctx);
              solution_queue.push(z3_sol);
            }
          }
        }
      } else {
        tb.submit_task_rust(&task, solution_queue.clone(), false, &inputs);
      }

       
      let used_t1 = t_start.elapsed().as_secs() as u32;
      if (used_t1 > 90)  { //3min
        break;
      }
    }
  }
  info!("submitted {} tasks {:?}", count, t_start.elapsed());
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
    /*
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
    */
  }
}
