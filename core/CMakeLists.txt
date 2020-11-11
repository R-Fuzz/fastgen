cmake_minimum_required(VERSION 3.5.1)

project(rgd C CXX)
# Proto file
get_filename_component(rgd_proto "rgd.proto" ABSOLUTE)
get_filename_component(rgd_proto_path "${rgd_proto}" PATH)

# Generated sources
set(rgd_proto_srcs "${CMAKE_CURRENT_BINARY_DIR}/rgd.pb.cc")
set(rgd_proto_hdrs "${CMAKE_CURRENT_BINARY_DIR}/rgd.pb.h")
add_custom_command(
  OUTPUT "${rgd_proto_srcs}" "${rgd_proto_hdrs}"
  COMMAND protoc
  ARGS  --cpp_out "${CMAKE_CURRENT_BINARY_DIR}"
    -I "${rgd_proto_path}"
    "${rgd_proto}"
  DEPENDS "${rgd_proto}")


include_directories("${CMAKE_CURRENT_BINARY_DIR}")

add_library(gd
  STATIC
  lib.cc
  util.cc
  ${rgd_proto_srcs}
)
