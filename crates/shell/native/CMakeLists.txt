cmake_minimum_required(VERSION 3.20)
project(cove-rust-shell-native LANGUAGES C CXX)
get_filename_component(COVE_ROOT "${CMAKE_CURRENT_LIST_DIR}/../../.." ABSOLUTE)
set(BUILD_SHARED_LIBS OFF CACHE BOOL "" FORCE)
set(CMAKE_POSITION_INDEPENDENT_CODE ON CACHE BOOL "" FORCE)
set(SDL_STATIC ON CACHE BOOL "" FORCE)
set(SDL_SHARED OFF CACHE BOOL "" FORCE)
set(SDL_TEST_LIBRARY OFF CACHE BOOL "" FORCE)
set(SDL_TESTS OFF CACHE BOOL "" FORCE)
set(SDL_INSTALL OFF CACHE BOOL "" FORCE)
add_subdirectory("${COVE_ROOT}/Native/third-party/sdl3" sdl3-build)
if(ANDROID)
    target_compile_definitions(SDL3-static PRIVATE JNI_OnLoad=CoveSDL_JNI_OnLoad)
endif()
add_custom_target(stage_sdl3
    DEPENDS SDL3-static
    COMMAND ${CMAKE_COMMAND} -E make_directory "${COVE_VENDOR_DIR}/SDL3.artifactbundle/${COVE_TRIPLE}/lib"
    COMMAND ${CMAKE_COMMAND} -E copy $<TARGET_FILE:SDL3-static> "${COVE_VENDOR_DIR}/SDL3.artifactbundle/${COVE_TRIPLE}/lib/"
    COMMAND ${CMAKE_COMMAND} -E copy_directory "${COVE_ROOT}/Native/third-party/sdl3/include/SDL3" "${COVE_VENDOR_DIR}/SDL3.artifactbundle/${COVE_TRIPLE}/include/SDL3")
