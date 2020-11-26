//use protoc_rust::Customize;

fn main() {
  protoc_rust::Codegen::new()
    .out_dir("src")
    .input("protos/rgd.proto")
    .include("protos")
    .run()
    .expect("protoc");

  println!(r"cargo:rustc-link-search=fastgen/searcher/build");
  println!(r"cargo:rustc-link-search=/usr/lib/llvm-9/lib");
}
