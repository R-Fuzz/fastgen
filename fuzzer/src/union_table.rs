#[repr(C,align(8))] 
pub struct dfsan_label_info {
  pub l1: u32,
  pub l2: u32,
  pub op1: u64,
  pub op2: u64,
  pub op: u16,
  pub size: u16,
  pub flags: u8,
  pub tree_size: u32,
  pub hash: u32,
  pub unused1: u64, //this is *expr 
  pub unused2: u64,
}

pub type UnionTable = [dfsan_label_info; 50331648];

