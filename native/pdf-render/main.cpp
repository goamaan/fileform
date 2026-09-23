// SPDX-License-Identifier: Apache-2.0
// One document/page per process. The Rust supervisor must enforce time/resource
// limits. This adapter is not a sandbox and does not establish PDF validity.
#include <fpdfview.h>
#include <fpdf_transformpage.h>
#include <algorithm>
#include <charconv>
#include <cmath>
#include <cstdint>
#include <cstdio>
#include <filesystem>
#include <fstream>
#include <iostream>
#include <stdexcept>
#include <string>
#include <vector>
#ifdef _WIN32
#include <fcntl.h>
#include <io.h>
#endif

namespace {
constexpr std::uintmax_t input_limit = 512ULL * 1024 * 1024;
int number(const std::string& value, int minimum, int maximum) {
  int result = 0;
  const auto parsed = std::from_chars(value.data(), value.data() + value.size(), result);
  if (parsed.ec != std::errc{} || parsed.ptr != value.data() + value.size() ||
      result < minimum || result > maximum)
    throw std::runtime_error("Invalid page index or image bound");
  return result;
}
struct Library {
  Library() {
    FPDF_LIBRARY_CONFIG config{};
    config.version = 2;
    FPDF_InitLibraryWithConfig(&config);
  }
  ~Library() { FPDF_DestroyLibrary(); }
};
struct Document {
  FPDF_DOCUMENT value;
  ~Document() { if (value) FPDF_CloseDocument(value); }
};
struct Page {
  FPDF_PAGE value;
  ~Page() { if (value) FPDF_ClosePage(value); }
};
struct Bitmap {
  FPDF_BITMAP value;
  ~Bitmap() { if (value) FPDFBitmap_Destroy(value); }
};
int render(const std::filesystem::path& input, int index, int edge, bool media) {
  if (!std::filesystem::is_regular_file(input))
    throw std::runtime_error("Input must be a regular file");
  std::ifstream source(input, std::ios::binary | std::ios::ate);
  const auto length = source.tellg();
  if (!source || length <= 0 || static_cast<std::uintmax_t>(length) > input_limit)
    throw std::runtime_error("PDF exceeds the input limit or cannot be read");
  std::vector<char> data(static_cast<std::size_t>(length));
  source.seekg(0);
  if (!source.read(data.data(), static_cast<std::streamsize>(data.size())))
    throw std::runtime_error("Cannot read PDF snapshot");
  Library library;
  Document document{FPDF_LoadMemDocument64(data.data(), data.size(), nullptr)};
  if (!document.value) throw std::runtime_error("Cannot open PDF");
  const int count = FPDF_GetPageCount(document.value);
  if (count < 1 || count > 1000 || index >= count)
    throw std::runtime_error("Page count or index outside limits");
  Page page{FPDF_LoadPage(document.value, index)};
  if (!page.value) throw std::runtime_error("Cannot load PDF page");
  if (media) {
    float left = 0, bottom = 0, right = 0, top = 0;
    if (!FPDFPage_GetMediaBox(page.value, &left, &bottom, &right, &top) ||
        !std::isfinite(left) || !std::isfinite(bottom) ||
        !std::isfinite(right) || !std::isfinite(top) || right <= left || top <= bottom)
      throw std::runtime_error("Cannot resolve PDF media box");
    // In-memory viewport only; no document is saved or modified on disk.
    FPDFPage_SetCropBox(page.value, left, bottom, right, top);
  }
  const double width = FPDF_GetPageWidthF(page.value);
  const double height = FPDF_GetPageHeightF(page.value);
  if (!std::isfinite(width) || !std::isfinite(height) || width <= 0 || height <= 0 ||
      width > 1000000 || height > 1000000)
    throw std::runtime_error("Unsupported page dimensions");
  const double scale = std::min(2.0, edge / std::max(width, height));
  const int w = std::clamp(static_cast<int>(std::ceil(width * scale)), 1, edge);
  const int h = std::clamp(static_cast<int>(std::ceil(height * scale)), 1, edge);
  const int stride = w * 4; // edge <= 2048; all products fit int and size_t.
  std::vector<unsigned char> pixels(static_cast<std::size_t>(stride) * h, 255);
  Bitmap bitmap{FPDFBitmap_CreateEx(w, h, FPDFBitmap_BGRA, pixels.data(), stride)};
  if (!bitmap.value || !FPDFBitmap_FillRect(bitmap.value, 0, 0, w, h, 0xffffffff))
    throw std::runtime_error("Cannot allocate PDF bitmap");
  // Intrinsic crop/rotation follows PDFium; no extra rotation and no form callbacks.
  FPDF_RenderPageBitmap(bitmap.value, page.value, 0, 0, w, h, 0, FPDF_ANNOT);
  std::vector<unsigned char> row(static_cast<std::size_t>(w) * 3);
  std::cout << "P6\n" << w << ' ' << h << "\n255\n";
  for (int y = 0; y < h; ++y) {
    for (int x = 0; x < w; ++x) {
      const auto offset = static_cast<std::size_t>(y) * stride + x * 4;
      row[x * 3] = pixels[offset + 2];
      row[x * 3 + 1] = pixels[offset + 1];
      row[x * 3 + 2] = pixels[offset];
    }
    std::cout.write(reinterpret_cast<const char*>(row.data()),
                    static_cast<std::streamsize>(row.size()));
  }
  std::cout.flush();
  if (!std::cout) throw std::runtime_error("Cannot write raster");
  return 0;
}
}

#ifdef _WIN32
int wmain(int argc, wchar_t** argv) {
  _setmode(_fileno(stdout), _O_BINARY);
#else
int main(int argc, char** argv) {
#endif
  try {
    if (argc != 4 && argc != 5) throw std::runtime_error("Usage: fileform-pdf-render SNAPSHOT PAGE_INDEX MAX_EDGE [crop|media]");
    const auto box = argc == 5 ? std::filesystem::path(argv[4]).string() : "crop";
    if (box != "crop" && box != "media") throw std::runtime_error("Choose crop or media box");
    return render(std::filesystem::path(argv[1]),
                  number(std::filesystem::path(argv[2]).string(), 0, 999),
                  number(std::filesystem::path(argv[3]).string(), 1, 2048), box == "media");
  } catch (const std::exception& error) {
    std::cerr << error.what() << '\n';
    return 1;
  }
}
