find_package(Boost COMPONENTS unit_test_framework REQUIRED)

file(GLOB test_srcs RELATIVE ${CMAKE_CURRENT_SOURCE_DIR} "*.cc")
foreach(test_src ${test_srcs})
  get_filename_component(test_name ${test_src} NAME_WE)

  # add normal executable
  add_executable(${test_name} ${test_src})
  target_link_libraries(${test_name}
    gd ${Boost_UNIT_TEST_FRAMEWORK_LIBRARY})
  set_target_properties(${test_name}  
    PROPERTIES RUNTIME_OUTPUT_DIRECTORY ${CMAKE_TEST_OUTPUT_DIRECTORY})
  add_test(
    NAME ${test_name}
    COMMAND "${CMAKE_TEST_OUTPUT_DIRECTORY}/${test_name}"
    WORKING_DIRECTORY "${CMAKE_TEST_OUTPUT_DIRECTORY}"
    )
endforeach()

