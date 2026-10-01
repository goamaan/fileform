// SPDX-License-Identifier: Apache-2.0
// Evaluation adapter. Decode outside Rust/UI; the supervisor supplies private
// snapshots and enforces cancellation/timeouts. Library limits are not OS isolation.
#include <libheif/heif.h>
#include <libheif/heif_items.h>
#include <libheif/heif_sequences.h>
#include <libheif/heif_properties.h>
#include <algorithm>
#include <cstdint>
#include <cstdio>
#include <filesystem>
#include <fstream>
#include <iostream>
#include <memory>
#include <stdexcept>
#include <string>
#include <vector>
#ifdef _WIN32
#include <fcntl.h>
#include <io.h>
#endif
namespace {
constexpr std::uintmax_t max_source = 512ULL*1024*1024;
constexpr std::uint64_t max_pixels = 80000000;
void require(bool valid, const char* message) { if (!valid) throw std::runtime_error(message); }
void checked(heif_error error) { require(error.code==heif_error_Ok,"HEIC parsing or decoding failed"); }
struct Library { Library(){checked(heif_init(nullptr));} ~Library(){heif_deinit();} };
struct Reader {
  std::ifstream input;
  std::int64_t length;
  Reader(const std::filesystem::path& path): input(path,std::ios::binary), length(0) {
    require(std::filesystem::is_regular_file(path),"Require a regular input file");
    const auto size=std::filesystem::file_size(path);
    require(input.good() && size>0 && size<=max_source,"Invalid or oversized HEIC input");
    length=static_cast<std::int64_t>(size);
  }
  static std::int64_t position(void* user) { return static_cast<std::int64_t>(static_cast<Reader*>(user)->input.tellg()); }
  static int read(void* data,std::size_t size,void* user) {
    auto& reader=*static_cast<Reader*>(user);const auto current=position(user);
    if(current<0 || current>reader.length || size>static_cast<std::uint64_t>(reader.length-current)) return 1;
    reader.input.read(static_cast<char*>(data),static_cast<std::streamsize>(size));return reader.input.good()?0:1;
  }
  static int seek(std::int64_t position,void* user) {
    auto& reader=*static_cast<Reader*>(user);if(position<0 || position>reader.length)return 1;
    reader.input.clear();reader.input.seekg(position);return reader.input.good()?0:1;
  }
  static heif_reader_grow_status wait(std::int64_t position,void* user) {
    return position>=0 && position<=static_cast<Reader*>(user)->length ? heif_reader_grow_status_size_reached : heif_reader_grow_status_size_beyond_eof;
  }
};
using Handle=std::unique_ptr<heif_image_handle,decltype(&heif_image_handle_release)>;
using Nclx=std::unique_ptr<heif_color_profile_nclx,decltype(&heif_nclx_color_profile_free)>;
Nclx profile(const heif_image_handle* handle) {
  heif_color_profile_nclx* raw=nullptr;const auto error=heif_image_handle_get_nclx_color_profile(handle,&raw);
  Nclx value(raw,heif_nclx_color_profile_free);
  require(error.code==heif_error_Ok || error.code==heif_error_Color_profile_does_not_exist,"Invalid HEIC color metadata");
  if(value) require(value->transfer_characteristics!=16 && value->transfer_characteristics!=18,"HDR HEIC requires a preservation workflow");
  return value;
}
Nclx profile(const heif_image* image) {
  heif_color_profile_nclx* raw=nullptr;const auto error=heif_image_get_nclx_color_profile(image,&raw);
  Nclx value(raw,heif_nclx_color_profile_free);
  require(error.code==heif_error_Ok || error.code==heif_error_Color_profile_does_not_exist,"Invalid decoded HEIC color metadata");
  if(value) require(value->transfer_characteristics!=16 && value->transfer_characteristics!=18,"Decoded HDR HEIC is unsupported");
  return value;
}
void auxiliaries(const heif_image_handle* handle) {
  const auto count=heif_image_handle_get_number_of_auxiliary_images(handle,0);require(count>=0 && count<=64,"Too many HEIC auxiliary images");
  std::vector<heif_item_id> ids(static_cast<std::size_t>(count));
  require(heif_image_handle_get_list_of_auxiliary_image_IDs(handle,0,ids.data(),count)==count,"Incomplete HEIC auxiliary inventory");
  for(const auto id:ids) {
    heif_image_handle* raw=nullptr;checked(heif_image_handle_get_auxiliary_image_handle(handle,id,&raw));Handle auxiliary(raw,heif_image_handle_release);
    require(auxiliary!=nullptr,"Missing HEIC auxiliary handle");
    const char* type=nullptr;checked(heif_image_handle_get_auxiliary_type(auxiliary.get(),&type));
    std::string name;
    if(type)for(std::size_t index=0;index<=4096 && type[index]!=0;++index)name.push_back(type[index]);
    heif_image_handle_release_auxiliary_type(auxiliary.get(),&type);
    require(name.size()<=4096,"HEIC auxiliary type exceeds its bound");
    require(name.find("hdrgainmap")==std::string::npos && name.find("21496")==std::string::npos && name.find("gainmap")==std::string::npos,"HEIC gain maps require a preservation workflow");
  }
}
void decode(const std::filesystem::path& path) {
  Library library;Reader reader(path);
  std::unique_ptr<heif_context,decltype(&heif_context_free)> context(heif_context_alloc(),heif_context_free);
  require(context!=nullptr,"Could not allocate HEIC context");
  auto* limits=heif_context_get_security_limits(context.get());require(limits!=nullptr && limits->version>=4,"HEIC security limits are unavailable");
  limits->max_image_size_pixels=max_pixels;limits->max_number_of_tiles=4096;limits->max_items=8192;
  limits->max_color_profile_size=4*1024*1024;limits->max_memory_block_size=max_source;
  limits->max_total_memory=1024ULL*1024*1024;limits->max_components=16;
  limits->max_iloc_extents_per_item=4096;limits->max_size_entity_group=1024;limits->max_children_per_box=8192;
  limits->max_sequence_frames=1;limits->max_number_of_file_brands=64;
  limits->max_sample_description_box_entries=64;limits->max_sample_group_description_box_entries=64;
  heif_context_set_max_decoding_threads(context.get(),0);
  heif_reader callbacks{};callbacks.reader_api_version=1;callbacks.get_position=Reader::position;callbacks.read=Reader::read;callbacks.seek=Reader::seek;callbacks.wait_for_file_size=Reader::wait;
  checked(heif_context_read_from_reader(context.get(),&callbacks,&reader,nullptr));
  require(!heif_context_has_sequence(context.get()) && heif_context_number_of_sequence_tracks(context.get())==0,"HEIC sequences are unsupported");
  require(heif_context_get_number_of_top_level_images(context.get())==1,"Choose a single HEIC image");
  const auto count=heif_context_get_number_of_items(context.get());require(count>0 && count<=8192,"Invalid HEIC item inventory");
  std::vector<heif_item_id> ids(static_cast<std::size_t>(count));
  require(heif_context_get_list_of_item_IDs(context.get(),ids.data(),count)==count,"Incomplete HEIC item inventory");
  for(const auto id:ids)require(heif_item_get_item_type(context.get(),id)!=heif_fourcc('t','m','a','p'),"HEIC tone-map items require a preservation workflow");
  heif_image_handle* raw_handle=nullptr;checked(heif_context_get_primary_image_handle(context.get(),&raw_handle));Handle handle(raw_handle,heif_image_handle_release);
  require(handle!=nullptr,"Missing primary HEIC image");
  const auto luma=heif_image_handle_get_luma_bits_per_pixel(handle.get());const auto chroma=heif_image_handle_get_chroma_bits_per_pixel(handle.get());
  require(luma>0 && luma<=8 && (chroma==-1 || (chroma>0 && chroma<=8)),"High-depth HEIC is unsupported");
  auto signalled=profile(handle.get());auxiliaries(handle.get());
  const auto profile_size=heif_image_handle_get_raw_color_profile_size(handle.get());require(profile_size<=4*1024*1024,"HEIC ICC profile is too large");
  std::vector<std::uint8_t> icc(profile_size);if(profile_size)checked(heif_image_handle_get_raw_color_profile(handle.get(),icc.data()));
  std::unique_ptr<heif_decoding_options,decltype(&heif_decoding_options_free)> options(heif_decoding_options_alloc(),heif_decoding_options_free);
  require(options!=nullptr && options->version>=10,"HEIC decode options are unavailable");
  options->strict_decoding=1;options->ignore_transformations=0;options->convert_hdr_to_8bit=0;
  options->output_image_nclx_profile_passthrough=1;options->num_codec_threads=2;
  options->color_conversion_options.preferred_chroma_upsampling_algorithm=heif_chroma_upsampling_bilinear;
  options->color_conversion_options.only_use_preferred_chroma_algorithm=1;
  const auto item=heif_image_handle_get_item_id(handle.get());
  const auto rotations=heif_item_get_properties_of_type(context.get(),item,heif_item_property_type_transform_rotation,nullptr,0);
  const auto mirrors=heif_item_get_properties_of_type(context.get(),item,heif_item_property_type_transform_mirror,nullptr,0);
  require(rotations>=0 && rotations<=1 && mirrors>=0 && mirrors<=1,"Unsupported HEIC orientation properties");
  const auto exif_count=heif_image_handle_get_number_of_metadata_blocks(handle.get(),"Exif");
  require(exif_count>=0 && exif_count<=1,"Ambiguous HEIC EXIF metadata");
  std::vector<std::uint8_t> exif;
  if(exif_count==1) {
    heif_item_id metadata_id=0;
    require(heif_image_handle_get_list_of_metadata_block_IDs(handle.get(),"Exif",&metadata_id,1)==1,"Missing HEIC EXIF metadata");
    const auto size=heif_image_handle_get_metadata_size(handle.get(),metadata_id);require(size<=65536,"HEIC EXIF exceeds its bound");
    exif.resize(size);if(size)checked(heif_image_handle_get_metadata(handle.get(),metadata_id,exif.data()));
  }
  heif_image* raw_image=nullptr;checked(heif_decode_image(handle.get(),&raw_image,heif_colorspace_RGB,heif_chroma_interleaved_RGBA,options.get()));
  std::unique_ptr<heif_image,decltype(&heif_image_release)> image(raw_image,heif_image_release);require(image!=nullptr,"HEIC decoding returned no image");
  require(heif_image_get_decoding_warnings(image.get(),0,nullptr,0)==0,"HEIC decoding returned warnings");
  auto decoded=profile(image.get());
  require(heif_image_get_bits_per_pixel_range(image.get(),heif_channel_interleaved)==8,"Decoded HEIC precision differs");
  const auto width=heif_image_get_width(image.get(),heif_channel_interleaved);const auto height=heif_image_get_height(image.get(),heif_channel_interleaved);
  require(width>0 && height>0 && static_cast<std::uint64_t>(width)*static_cast<std::uint64_t>(height)<=max_pixels,"Decoded HEIC dimensions exceed bounds");
  require(width==heif_image_get_primary_width(image.get()) && height==heif_image_get_primary_height(image.get()),"HEIC plane and primary dimensions differ");
  std::size_t stride=0;const auto* pixels=heif_image_get_plane_readonly2(image.get(),heif_channel_interleaved,&stride);
  const auto row=static_cast<std::size_t>(width)*4;require(pixels!=nullptr && stride>=row && stride<=max_source/static_cast<std::size_t>(height),"Invalid HEIC plane stride");
  const auto decoded_icc_size=heif_image_get_raw_color_profile_size(image.get());require(decoded_icc_size==profile_size,"HEIC ICC ownership changed during decode");
  if(profile_size){std::vector<std::uint8_t> actual(profile_size);checked(heif_image_get_raw_color_profile(image.get(),actual.data()));require(actual==icc,"HEIC ICC profile changed during decode");}
  // FH1 reports source color metadata; color conversion belongs to Rust. Header
  // and ICC are followed by tightly packed RGBA rows, without any stride padding.
  const auto* color=decoded?decoded.get():signalled.get();
  std::cout<<"FH1\n"<<width<<' '<<height<<' '<<heif_image_handle_has_alpha_channel(handle.get())<<' '<<heif_image_is_premultiplied_alpha(image.get())<<' '<<icc.size()<<' '<<(color?static_cast<int>(color->color_primaries):0)<<' '<<(color?static_cast<int>(color->transfer_characteristics):0)<<' '<<exif.size()<<' '<<(rotations+mirrors>0)<<"\n";
  if(!icc.empty())std::cout.write(reinterpret_cast<const char*>(icc.data()),static_cast<std::streamsize>(icc.size()));
  if(!exif.empty())std::cout.write(reinterpret_cast<const char*>(exif.data()),static_cast<std::streamsize>(exif.size()));
  for(int y=0;y<height;++y)std::cout.write(reinterpret_cast<const char*>(pixels+static_cast<std::size_t>(y)*stride),static_cast<std::streamsize>(row));
  std::cout.flush();require(std::cout.good(),"Could not write HEIC decoded output");
}
}
#ifdef _WIN32
int wmain(int count,wchar_t** args) {
  _setmode(_fileno(stdout),_O_BINARY);
#else
int main(int count,char** args) {
#endif
  try {require(count==2,"Pass one private HEIC source snapshot");decode(std::filesystem::path(args[1]));return 0;}
  catch(const std::exception& error){std::cerr<<error.what()<<'\n';return 2;}
}
