// SPDX-License-Identifier: Apache-2.0
// One document/page per process. The Rust supervisor must enforce time/resource
// limits. This adapter is not a sandbox and does not establish PDF validity.
#include <fpdfview.h>
#include <fpdf_transformpage.h>
#include <fpdf_text.h>
#include <fpdf_edit.h>
#include <fpdf_formfill.h>
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
// Static rendering only: no timers, navigation, user interaction or JavaScript
// platform. The non-V8/non-XFA dependency is enforced by the build recipe.
struct FormInfo : FPDF_FORMFILLINFO {
  FPDF_DOCUMENT document;
  FPDF_PAGE page;
  int index;
  FormInfo(FPDF_DOCUMENT d, FPDF_PAGE p, int i)
      : FPDF_FORMFILLINFO{}, document(d), page(p), index(i) {
    version = 1;
    FFI_Invalidate = [](FPDF_FORMFILLINFO*, FPDF_PAGE, double, double, double, double) {};
    FFI_SetCursor = [](FPDF_FORMFILLINFO*, int) {};
    FFI_SetTimer = [](FPDF_FORMFILLINFO*, int, TimerCallback) { return 0; };
    FFI_KillTimer = [](FPDF_FORMFILLINFO*, int) {};
    FFI_GetLocalTime = [](FPDF_FORMFILLINFO*) { return FPDF_SYSTEMTIME{}; };
    FFI_GetPage = [](FPDF_FORMFILLINFO* info, FPDF_DOCUMENT requested_document, int requested_index) {
      const auto* self = static_cast<FormInfo*>(info);
      return requested_document == self->document && requested_index == self->index ? self->page : nullptr;
    };
    FFI_GetCurrentPage = [](FPDF_FORMFILLINFO* info, FPDF_DOCUMENT requested_document) {
      const auto* self = static_cast<FormInfo*>(info);
      return requested_document == self->document ? self->page : nullptr;
    };
    FFI_GetRotation = [](FPDF_FORMFILLINFO*, FPDF_PAGE requested_page) { return FPDFPage_GetRotation(requested_page); };
    FFI_ExecuteNamedAction = [](FPDF_FORMFILLINFO*, FPDF_BYTESTRING) {};
  }
};
struct FormEnvironment {
  FormInfo info;
  FPDF_FORMHANDLE handle = nullptr;
  FormEnvironment(const FormEnvironment&) = delete;
  FormEnvironment& operator=(const FormEnvironment&) = delete;
  explicit FormEnvironment(FPDF_DOCUMENT document, FPDF_PAGE page, int index)
      : info(document, page, index) {
    handle = FPDFDOC_InitFormFillEnvironment(document, &info);
    if (!handle) throw std::runtime_error("Cannot initialize static form appearances");
    FORM_OnAfterLoadPage(page, handle);
  }
  ~FormEnvironment() {
    if (handle) {
      FORM_OnBeforeClosePage(info.page, handle);
      FPDFDOC_ExitFormFillEnvironment(handle);
    }
  }
};
struct TextPage {
  FPDF_TEXTPAGE value;
  ~TextPage() { if (value) FPDFText_ClosePage(value); }
};
void append_utf8(std::string& output, unsigned int value) {
  if (value == 0 || value > 0x10ffff || (value >= 0xd800 && value <= 0xdfff))
    throw std::runtime_error("PDF text contains an unmapped or invalid Unicode character");
  if (value <= 0x7f) output.push_back(static_cast<char>(value));
  else if (value <= 0x7ff) {
    output.push_back(static_cast<char>(0xc0 | (value >> 6)));
    output.push_back(static_cast<char>(0x80 | (value & 0x3f)));
  } else if (value <= 0xffff) {
    output.push_back(static_cast<char>(0xe0 | (value >> 12)));
    output.push_back(static_cast<char>(0x80 | ((value >> 6) & 0x3f)));
    output.push_back(static_cast<char>(0x80 | (value & 0x3f)));
  } else {
    output.push_back(static_cast<char>(0xf0 | (value >> 18)));
    output.push_back(static_cast<char>(0x80 | ((value >> 12) & 0x3f)));
    output.push_back(static_cast<char>(0x80 | ((value >> 6) & 0x3f)));
    output.push_back(static_cast<char>(0x80 | (value & 0x3f)));
  }
}
int extract_text(FPDF_PAGE page) {
  TextPage text{FPDFText_LoadPage(page)};
  if (!text.value) throw std::runtime_error("Cannot inspect PDF text");
  const int count = FPDFText_CountChars(text.value);
  if (count < 0 || count > 1000000) throw std::runtime_error("PDF text exceeds the character limit");
  std::string output;
  output.reserve(static_cast<std::size_t>(count) * 4);
  int scalars = 0;
  for (int i = 0; i < count; ++i) {
    if (FPDFText_HasUnicodeMapError(text.value, i) != 0)
      throw std::runtime_error("PDF text has an invalid Unicode mapping");
    unsigned int value = FPDFText_GetUnicode(text.value, i);
    // Some ToUnicode mappings are exposed as UTF-16 surrogate entries even
    // though GetUnicode returns an unsigned int. Join only a valid adjacent pair.
    if (value >= 0xd800 && value <= 0xdbff) {
      if (++i >= count || FPDFText_HasUnicodeMapError(text.value, i) != 0)
        throw std::runtime_error("PDF text contains an incomplete Unicode pair");
      const unsigned int low = FPDFText_GetUnicode(text.value, i);
      if (low < 0xdc00 || low > 0xdfff)
        throw std::runtime_error("PDF text contains an invalid Unicode pair");
      value = 0x10000 + ((value - 0xd800) << 10) + (low - 0xdc00);
    }
    append_utf8(output, value);
    ++scalars;
  }
  // Framing lets Rust distinguish valid empty text from missing/truncated output.
  std::cout << "FT1\n" << scalars << ' ' << output.size() << '\n';
  std::cout.write(output.data(), static_cast<std::streamsize>(output.size()));
  std::cout.flush();
  if (!std::cout) throw std::runtime_error("Cannot write extracted text");
  return 0;
}
int render(const std::filesystem::path& input, int index, int edge, bool media,
           const std::vector<float>& resolved_box, int rotation, bool ocr) {
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
  if (edge == 0) return extract_text(page.value);
  if (rotation >= 0) FPDFPage_SetRotation(page.value, rotation);
  if (media) {
    float left = 0, bottom = 0, right = 0, top = 0;
    if (!resolved_box.empty()) {
      left = resolved_box[0]; bottom = resolved_box[1];
      right = resolved_box[2]; top = resolved_box[3];
      FPDFPage_SetMediaBox(page.value, left, bottom, right, top);
    } else if (!FPDFPage_GetMediaBox(page.value, &left, &bottom, &right, &top)) {
      throw std::runtime_error("Cannot resolve PDF media box");
    }
    if (
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
  FormEnvironment forms(document.value, page.value, index);
  const double scale = ocr ? edge / std::max(width, height) : std::min(2.0, edge / std::max(width, height));
  const int w = std::clamp(static_cast<int>(std::ceil(width * scale)), 1, edge);
  const int h = std::clamp(static_cast<int>(std::ceil(height * scale)), 1, edge);
  const int stride = w * 4; // edge <= 4096; all products fit int and size_t.
  std::vector<unsigned char> pixels(static_cast<std::size_t>(stride) * h, 255);
  Bitmap bitmap{FPDFBitmap_CreateEx(w, h, FPDFBitmap_BGRA, pixels.data(), stride)};
  if (!bitmap.value || !FPDFBitmap_FillRect(bitmap.value, 0, 0, w, h, 0xffffffff))
    throw std::runtime_error("Cannot allocate PDF bitmap");
  // Intrinsic crop/rotation follows PDFium; no extra rotation and no form callbacks.
  FPDF_RenderPageBitmap(bitmap.value, page.value, 0, 0, w, h, 0, FPDF_ANNOT);
  FPDF_FFLDraw(forms.handle, bitmap.value, page.value, 0, 0, w, h, 0, FPDF_ANNOT);
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
    if (argc != 4 && argc != 5 && argc != 9 && argc != 10) throw std::runtime_error("Usage: fileform-pdf-render SNAPSHOT PAGE_INDEX {text|ocr|MAX_EDGE [crop|media [LEFT BOTTOM RIGHT TOP [QUARTER_TURNS]]]}");
    const auto operation = std::filesystem::path(argv[3]).string();
    if (operation == "text" && argc != 4) throw std::runtime_error("Text extraction does not accept a box option");
    const auto box = argc >= 5 ? std::filesystem::path(argv[4]).string() : (operation == "ocr" ? "media" : "crop");
    if (operation == "ocr" && box != "media") throw std::runtime_error("OCR requires the full media box");
    if (box != "crop" && box != "media") throw std::runtime_error("Choose crop or media box");
    std::vector<float> resolved_box;
    if (argc >= 9) {
      if (box != "media") throw std::runtime_error("Resolved coordinates require media mode");
      for (int i = 5; i < 9; ++i) {
        const auto value = std::filesystem::path(argv[i]).string();
        std::size_t used = 0;
        const auto coordinate = std::stof(value, &used);
        if (used != value.size() || !std::isfinite(coordinate))
          throw std::runtime_error("Invalid resolved media box coordinate");
        resolved_box.push_back(coordinate);
      }
    }
    return render(std::filesystem::path(argv[1]),
                  number(std::filesystem::path(argv[2]).string(), 0, 999),
                  operation == "text" ? 0 : (operation == "ocr" ? 4096 : number(operation, 1, 2048)), box == "media", resolved_box,
                  argc == 10 ? number(std::filesystem::path(argv[9]).string(), 0, 3) : -1, operation == "ocr");
  } catch (const std::exception& error) {
    std::cerr << error.what() << '\n';
    return 1;
  }
}
