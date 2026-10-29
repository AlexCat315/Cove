cmake_minimum_required(VERSION 3.20)
project(cove-rust-text-native LANGUAGES C CXX)
include(ExternalProject)
get_filename_component(COVE_ROOT "${CMAKE_CURRENT_LIST_DIR}/../.." ABSOLUTE)
set(COVE_TEXT_PREFIX "${CMAKE_BINARY_DIR}/install" CACHE PATH "Native dependency prefix")
set(_platform)
foreach(_name CMAKE_SYSTEM_NAME CMAKE_SYSTEM_PROCESSOR CMAKE_TOOLCHAIN_FILE CMAKE_OSX_SYSROOT
        CMAKE_OSX_ARCHITECTURES CMAKE_OSX_DEPLOYMENT_TARGET CMAKE_C_COMPILER CMAKE_CXX_COMPILER
        CMAKE_C_FLAGS CMAKE_CXX_FLAGS CMAKE_AR CMAKE_MSVC_RUNTIME_LIBRARY ANDROID_ABI ANDROID_PLATFORM ANDROID_STL)
    if(DEFINED ${_name} AND NOT "${${_name}}" STREQUAL "")
        list(APPEND _platform "-D${_name}=${${_name}}")
    endif()
endforeach()
if(MSVC)
    set(_freetype "freetype.lib")
else()
    set(_freetype "libfreetype.a")
endif()
ExternalProject_Add(cove_freetype
    SOURCE_DIR "${COVE_ROOT}/third-party/freetype"
    PREFIX "${CMAKE_BINARY_DIR}/freetype"
    CMAKE_ARGS ${_platform} -DCMAKE_INSTALL_PREFIX=${COVE_TEXT_PREFIX} -DCMAKE_BUILD_TYPE=Release
        -DCMAKE_POSITION_INDEPENDENT_CODE=ON -DBUILD_SHARED_LIBS=OFF
        -DFT_DISABLE_ZLIB=ON -DFT_DISABLE_BZIP2=ON -DFT_DISABLE_PNG=ON -DFT_DISABLE_HARFBUZZ=ON -DFT_DISABLE_BROTLI=ON)
ExternalProject_Add(cove_harfbuzz
    SOURCE_DIR "${COVE_ROOT}/third-party/harfbuzz"
    PREFIX "${CMAKE_BINARY_DIR}/harfbuzz"
    DEPENDS cove_freetype
    CMAKE_ARGS ${_platform} -DCMAKE_INSTALL_PREFIX=${COVE_TEXT_PREFIX} -DCMAKE_BUILD_TYPE=Release
        -DCMAKE_POSITION_INDEPENDENT_CODE=ON -DBUILD_SHARED_LIBS=OFF
        -DFREETYPE_LIBRARY=${COVE_TEXT_PREFIX}/lib/${_freetype}
        -DFREETYPE_INCLUDE_DIR_ft2build=${COVE_TEXT_PREFIX}/include/freetype2
        -DFREETYPE_INCLUDE_DIR_freetype2=${COVE_TEXT_PREFIX}/include/freetype2
        -DHB_HAVE_FREETYPE=ON -DHB_BUILD_TESTS=OFF -DHB_BUILD_UTILS=OFF -DHB_BUILD_SUBSET=OFF
        -DHB_HAVE_GLIB=OFF -DHB_HAVE_ICU=OFF -DHB_HAVE_GRAPHITE2=OFF -DHB_HAVE_CORETEXT=OFF)
