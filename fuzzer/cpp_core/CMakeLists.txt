cmake_minimum_required(VERSION 3.5.1)

project(rgd C CXX)

## set up test
set(CMAKE_CXX_STANDARD 14)
set(CMAKE_CXX_FLAGS "${CMAKE_CXX_FLAGS} -g -O3 -std=c++14 -fPIC -mcx16 -march=native -L/usr/local/lib")

enable_testing()
set(CMAKE_TEST_OUTPUT_DIRECTORY ${CMAKE_BINARY_DIR}/bin/test)
set(CMAKE_TEST_INCLUDE_DIR ${CMAKE_CURRENT_SOURCE_DIR})

find_package(LLVM 9 REQUIRED CONFIG)

if (LLVM_FOUND)
  message(STATUS "Found LLVM ${LLVM_PACKAGE_VERSION}")
  message(STATUS "Using LLVMConfig.cmake in: ${LLVM_DIR}")
else()
  message(FATAL_ERROR "You haven't install LLVM !")
endif()

include_directories(${LLVM_INCLUDE_DIRS})
add_definitions(${LLVM_DEFINITIONS})

# Proto file
get_filename_component(rgd_proto "../protos/rgd.proto" ABSOLUTE)
get_filename_component(rgd_proto_path "${rgd_proto}" PATH)

# Generated sources
add_custom_target(proto ALL DEPENDS ${rgd_proto_srcs} ${rgd_proto_hdrs})
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
  interface.cc
  jit.cc
  util.cc
  parser.cc
  gd.cc
  input.cc
  grad.cc
  z3solver.cc
  ${rgd_proto_srcs}
)

target_link_libraries(gd
  LLVM
  tcmalloc
  z3
  pthread)

add_subdirectory(self_test)
