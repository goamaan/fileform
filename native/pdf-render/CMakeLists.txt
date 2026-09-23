cmake_minimum_required(VERSION 3.22)
project(fileform_pdf_render LANGUAGES CXX)

# Evaluation build: PDFIUM_ROOT must point to a verified, non-V8 archive.
set(PDFIUM_ROOT "" CACHE PATH "Verified PDFium distribution")
if(NOT EXISTS "${PDFIUM_ROOT}/include/fpdfview.h")
  message(FATAL_ERROR "PDFIUM_ROOT must contain the matching PDFium headers")
endif()
file(READ "${PDFIUM_ROOT}/args.gn" PDFIUM_ARGS)
foreach(FEATURE pdf_enable_v8 pdf_enable_xfa)
  if(NOT PDFIUM_ARGS MATCHES "${FEATURE} = false")
    message(FATAL_ERROR "Require a non-V8/non-XFA PDFium build")
  endif()
endforeach()
add_executable(fileform-pdf-render main.cpp)
target_compile_features(fileform-pdf-render PRIVATE cxx_std_17)
target_include_directories(fileform-pdf-render PRIVATE "${PDFIUM_ROOT}/include")
if(WIN32)
  target_compile_options(fileform-pdf-render PRIVATE /W4 /WX /utf-8)
  target_link_libraries(fileform-pdf-render PRIVATE "${PDFIUM_ROOT}/lib/pdfium.dll.lib")
  set(PDFIUM_LIBRARY "${PDFIUM_ROOT}/bin/pdfium.dll")
elseif(APPLE)
  target_compile_options(fileform-pdf-render PRIVATE -Wall -Wextra -Werror)
  set(PDFIUM_LIBRARY "${PDFIUM_ROOT}/lib/libpdfium.dylib")
  target_link_libraries(fileform-pdf-render PRIVATE "${PDFIUM_LIBRARY}")
  add_custom_command(TARGET fileform-pdf-render POST_BUILD
    COMMAND install_name_tool -change ./libpdfium.dylib
      @executable_path/libpdfium.dylib $<TARGET_FILE:fileform-pdf-render>)
else()
  message(FATAL_ERROR "Only macOS and Windows are currently evaluated")
endif()
add_custom_command(TARGET fileform-pdf-render POST_BUILD
  COMMAND ${CMAKE_COMMAND} -E copy_if_different "${PDFIUM_LIBRARY}"
    $<TARGET_FILE_DIR:fileform-pdf-render>)
