#[repr(C,packed)] 
pub struct dfsan_label_info {
  pub l1: u32,
  pub l2: u32,
  pub op1: u64,
  pub op2: u64,
  pub op: u16,
  pub size: u16,
  pub hash: u32,
  pub tree_size: u32,
  pub depth: u32,
  pub flags: u8,
  pub padding1: u8,
  pub padding2: u8,
  pub padding3: u8,
  pub padding4: u8,
  pub padding5: u8,
  pub padding6: u8,
  pub padding7: u8,
}

pub type UnionTable = [dfsan_label_info; 50331648];

