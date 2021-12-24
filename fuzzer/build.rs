//use protoc_rust::Customize;

fn main() {
  protoc_rust::Codegen::new()
    .out_dir("src")
    .input("protos/rgd.proto")
    .include("protos")
    .run()
    .expect("protoc");


//  println!(r"cargo:rustc-link-search=fuzzer/cpp_core/build");
//  println!(r"cargo:rustc-link-search=/usr/lib/llvm-9/lib");
  println!(r"cargo:rustc-link-search=/usr/local/lib");
  println!(r"cargo:rustc-link-search=/out/lib");

}
