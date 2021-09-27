use std::collections::HashMap;
pub struct Solution {
  pub sol: HashMap<u32,u8>,
  pub fid: u32, 
  pub addr: u64,
  pub ctx: u64,
  pub order: u32,
  pub direction: u64,
  pub field_index: usize,
  pub field_size: usize,
  pub bid: u32,
  pub sctx: u32,
  pub is_cmp: bool,
  pub predicate: u32,
  pub target_cond: u64,
}

impl Solution {
  pub fn new(sol: HashMap<u32,u8>, fid: u32, addr: u64, 
            ctx: u64, order: u32, direction: u64, 
            field_index: usize, field_size: usize,
            bid: u32, sctx: u32, is_cmp: bool, predicate: u32, target_cond:u64) -> Self {
    Self {
      sol: sol,
      fid: fid,
      addr: addr,
      ctx: ctx, 
      order: order,
      direction: direction,
      field_index: field_index,
      field_size: field_size,
      bid: bid,
      sctx: sctx,
      is_cmp: is_cmp,
      predicate: predicate,
      target_cond: target_cond,
    }
  }
}
