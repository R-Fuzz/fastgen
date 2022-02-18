cmake_minimum_required(VERSION 3.5.1)

project(rgd C CXX)

## set up test
set(CMAKE_CXX_STANDARD 14)
set(CMAKE_CXX_FLAGS "${CMAKE_CXX_FLAGS} -g -O3 -std=c++14 -fPIC")

include_directories(${LLVM_INCLUDE_DIRS} "/out/include")
add_definitions(${LLVM_DEFINITIONS})

include_directories("${CMAKE_CURRENT_BINARY_DIR}")


add_library(gd
  STATIC
  filter.cc
)
