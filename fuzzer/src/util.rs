use crate::rgd::*;
use crate::op_def::*;
use num_traits::FromPrimitive;
use protobuf::Message; 
use protobuf::CodedInputStream;
use protobuf::CodedOutputStream;
use std::fs::File;
use std::io::prelude::*;
use std::path::Path;
use std::fs::OpenOptions;
use std::io::BufWriter;
use std::io::BufReader;
use crate::search_task::SearchTask;

pub fn to_rgd_op(op: u32) -> u32 { match op {
    DFSAN_BVEQ => RGD::Equal as u32,
    DFSAN_BVNEQ => RGD::Distinct as u32,
    DFSAN_BVSGT => RGD::Sgt as u32,
    DFSAN_BVSGE => RGD::Sge as u32,
    DFSAN_BVSLT => RGD::Slt as u32,
    DFSAN_BVSLE => RGD::Sle as u32,
    DFSAN_BVUGT => RGD::Ugt as u32,
    DFSAN_BVUGE => RGD::Uge as u32,
    DFSAN_BVULT => RGD::Ult as u32,
    DFSAN_BVULE => RGD::Ule as u32,
    _ => 0,
  }
}

fn get_name(op: u32) -> String {
  match FromPrimitive::from_u32(op) {
    Some(RGD::Equal) => "equal".to_string(),
    Some(RGD::Distinct) => "distinct".to_string(),
    Some(RGD::Sgt) => "sgt".to_string(),
    Some(RGD::Sge) => "sge".to_string(),
    Some(RGD::Slt) => "slt".to_string(),
    Some(RGD::Sle) => "sle".to_string(),
    Some(RGD::Ugt) => "ugt".to_string(),
    Some(RGD::Uge) => "uge".to_string(),
    Some(RGD::Ult) => "ult".to_string(),
    Some(RGD::Ule) => "ule".to_string(),
    Some(RGD::Bool) => "bool".to_string(),
    Some(RGD::Constant) => "constant".to_string(),
    Some(RGD::Read) => "read".to_string(),
    Some(RGD::Concat) => "concat".to_string(),
    Some(RGD::Extract) => "extract".to_string(),
    Some(RGD::ZExt) => "zext".to_string(),
    Some(RGD::SExt) => "sext".to_string(),
    Some(RGD::Add) => "add".to_string(),
    Some(RGD::Sub) => "sub".to_string(),
    Some(RGD::Mul) => "mul".to_string(),
    Some(RGD::UDiv) => "udiv".to_string(),
    Some(RGD::SDiv) => "sdiv".to_string(),
    Some(RGD::URem) => "urem".to_string(),
    Some(RGD::SRem) => "srem".to_string(),
    Some(RGD::Neg) => "neg".to_string(),
    Some(RGD::Not) => "not".to_string(),
    Some(RGD::And) => "and".to_string(),
    Some(RGD::Or) => "or".to_string(),
    Some(RGD::Xor) => "xor".to_string(),
    Some(RGD::Shl) => "shl".to_string(),
    Some(RGD::LShr) => "lshr".to_string(),
    Some(RGD::AShr) => "ashr".to_string(),
    Some(RGD::LNot) => "lnot".to_string(),
    Some(RGD::LAnd) => "land".to_string(),
    Some(RGD::LOr) => "lor".to_string(),
    Some(RGD::Uninit) => "Uninit".to_string(),
    _ => "".to_string(),
  }
}


pub fn do_print(node: &AstNode) {
  print!("{}(", get_name(node.get_kind()));
  print!("width={},",node.get_bits());
  print!("label={},",node.get_label());
  match FromPrimitive::from_u32(node.get_kind()) {
    Some(RGD::Bool) => print!("{}",node.get_value()),
    Some(RGD::Constant) => print!("{},",node.get_value()),
    Some(RGD::Memcmp) => print!("{},",node.get_value()),
    Some(RGD::Read) => print!("{},",node.get_index()),
    Some(RGD::Extract) => print!("{},",node.get_index()),
    _ => (),
  }
  for c in node.get_children() {
    do_print(c);
    print!(", ");
  }
  print!(")");
}

pub fn print_node(node: &AstNode) {
  do_print(node);
  println!("");
}

pub fn print_task(task: &SearchTask) {
  for cons_set in &task.flip_cons.0 {
    for cons in cons_set {
    println!("constraint label is {}", cons.get_label());
    print_node(cons.get_node());
/*
    for ainput in cons.get_meta().get_inputs() {
      println!("offset {} iv {}", ainput.get_offset(), ainput.get_iv());
    }
    for aarg in cons.get_meta().get_args() {
      println!("isinput {} iv {}", aarg.get_isinput(), aarg.get_v());
    }
    for amap in cons.get_meta().get_map() {
      println!("offset {} iv {}", amap.get_k(), amap.get_v());
    }
*/
  }
  }
}

pub fn save_request<M: Message>(message: &M, p: &Path) -> std::io::Result<()> {
  //open file for write
  let file = OpenOptions::new().append(true).create(true).open(p)?;
  let mut buf_writer = BufWriter::new(file); 
  let mut outstream = CodedOutputStream::new(&mut buf_writer);
  println!("write message with size {}", message.compute_size());
  message.write_length_delimited_to(&mut outstream);
  outstream.flush()?;
  Ok(())
}

pub fn load_request<M: Message>(p: &Path) -> std::io::Result<Vec<M>> {
  let file = File::open(p)?;
  let mut buf_reader = BufReader::new(file);
  let mut instream = CodedInputStream::new(&mut buf_reader);
  instream.set_recursion_limit(10000);
  //let size = instream.read_raw_varint32()?;
  //let limit = instream.push_limit(size.into())?; 
  let mut res = Vec::new();
  while let Ok(msg) = instream.read_message() {
    res.push(msg);
  }
  Ok(res)
}


#[inline(always)]
pub fn xxhash(h1: u32, h2: u32, h3: u32) -> u32 {
  //const PRIME32_1: u32 = 2654435761;
  const PRIME32_2: u32 = 2246822519u32;
  const PRIME32_3: u32 = 3266489917u32;
  const PRIME32_4: u32 =  668265263u32;
  const PRIME32_5: u32 =  374761393u32;

  let mut h32: u32 = PRIME32_5;
  h32 = h32.overflowing_add(h1.overflowing_mul(PRIME32_3).0).0;
  h32 = (h32 << 17 | h32 >> 15).overflowing_mul(PRIME32_4).0;
  h32 = h32.overflowing_add(h2.overflowing_mul(PRIME32_3).0).0;
  h32  = (h32 << 17 | h32 >> 15).overflowing_mul(PRIME32_4).0;
  h32 = h32.overflowing_add(h3.overflowing_mul(PRIME32_3).0).0;
  h32  = (h32 << 17 | h32 >> 15).overflowing_mul(PRIME32_4).0;

  h32 ^= h32 >> 15;
  h32 = h32.overflowing_mul(PRIME32_2).0;
  h32 ^= h32 >> 13;
  h32 = h32.overflowing_mul(PRIME32_3).0;
  h32 ^= h32 >> 16;

  h32
}

