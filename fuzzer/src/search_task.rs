use std::collections::HashMap;
use crate::rgd::*;
use std::rc::Rc;
pub struct SearchTask {
  //
  pub flip_cons: (Vec<Vec<Rc<Constraint>>>, bool), //flip
  pub path_cons: (Vec<Vec<Rc<Constraint>>>,bool), //path
  pub fid: u32, 
  pub addr: u64,
  pub ctx: u64,
  pub order: u32,
  pub direction: u64,
}

impl SearchTask {
  pub fn new(flip_cons: (Vec<Vec<Rc<Constraint>>>,bool), 
            path_cons: (Vec<Vec<Rc<Constraint>>>,bool),
            fid: u32, addr: u64, 
            ctx: u64, order: u32, direction: u64) -> Self {
    Self {
      flip_cons: flip_cons,
      path_cons: path_cons,
      fid: fid,
      addr: addr,
      ctx: ctx, 
      order: order,
      direction: direction,
    }
  }
}
