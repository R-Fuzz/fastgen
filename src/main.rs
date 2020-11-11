use libc;
use quickgen::rgd::*;
use quickgen::union_to_ast::*;
use quickgen::union_table::*;
use quickgen::util::*;
use protobuf::Message;


#[link(name = "gd")]
#[link(name = "protobuf")]
#[link(name = "stdc++")]
extern {
    fn print_buffer(input: *const u8, input_length: u32);
}

fn main() {
  let id = unsafe {
    libc::shmget(
        0x1234,
        0xc00000000, 
        0644 | libc::IPC_CREAT | libc::SHM_NORESERVE
        )
  };
  let ptr = unsafe { libc::shmat(id, std::ptr::null(), 0) as *mut UnionTable};
  let table = unsafe { & *ptr };
  let loc1 = &table[42];
  println!("l1 is {:?}", loc1.l1);
  let mut cmd = JitCmdv2::new();
  let mut req = JitRequest::new();

  union_to_ast(42,&mut req, table);

  cmd.mut_expr().push(req);
  print_req(&cmd.get_expr()[0]);

  let ast_node = cmd.write_to_bytes().unwrap();
  let astr = &ast_node;
  println!("{:?}",ast_node);
  unsafe { print_buffer(astr.as_ptr(), astr.len() as u32); }
}
