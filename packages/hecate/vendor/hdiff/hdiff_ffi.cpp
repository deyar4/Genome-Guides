#include "match_block.h"
#include "../hpatch/patch.h"

#include <cstdlib>
#include <vector>

static const size_t kPatchCacheSize_min = hpatch_kStreamCacheSize * 8;
static const size_t kPatchCacheSize_bestmin = (size_t)1 << 21;
static const size_t kPatchCacheSize_default = (size_t)1 << 22;
static const size_t _kNULL_SIZE = (~(size_t)0);

static unsigned char* getPatchMemCache(bool isLoadOldAll,
                                       size_t patchCacheSize,
                                       size_t mustAppendMemSize,
                                       hpatch_StreamPos_t oldDataSize,
                                       size_t* out_memCacheSize) {
    unsigned char* temp_cache = 0;
    size_t temp_cache_size;
    hpatch_StreamPos_t limitCacheSize;
    const hpatch_StreamPos_t bestMaxCacheSize = oldDataSize + kPatchCacheSize_bestmin;
    if (isLoadOldAll) {
        limitCacheSize = bestMaxCacheSize;
    } else {
        limitCacheSize = (patchCacheSize < kPatchCacheSize_min) ? kPatchCacheSize_min : patchCacheSize;
        limitCacheSize = (limitCacheSize < bestMaxCacheSize) ? limitCacheSize : bestMaxCacheSize;
    }
    if (limitCacheSize > (size_t)(_kNULL_SIZE - mustAppendMemSize))  // too large
        limitCacheSize = (size_t)(_kNULL_SIZE - mustAppendMemSize);
    temp_cache_size = (size_t)limitCacheSize;

    while (temp_cache_size >= kPatchCacheSize_min) {
        temp_cache = (unsigned char*)std::malloc(mustAppendMemSize + temp_cache_size);
        if (temp_cache) break;
        temp_cache_size >>= 1;
    }
    *out_memCacheSize = (temp_cache) ? (mustAppendMemSize + temp_cache_size) : 0;
    return temp_cache;
}

extern "C" {

struct HdiffBuffer {
    unsigned char* data;
    size_t len;
    void* handle;
    int status;  // 0 on success, non-zero on failure
};

typedef int (*HdiffReadCallback)(const void* user_data,
                                 hpatch_StreamPos_t read_from_pos,
                                 unsigned char* out_data,
                                 unsigned char* out_data_end);
typedef int (*HdiffWriteCallback)(void* user_data,
                                  hpatch_StreamPos_t write_to_pos,
                                  const unsigned char* data,
                                  const unsigned char* data_end);

struct HdiffIOCallbacks {
    const void* read_user;
    HdiffReadCallback read_fn;
    void* write_user;
    HdiffWriteCallback write_fn;
};

HdiffBuffer hdiff_create_single_compressed_diff_block(unsigned char* new_data,
                                                      size_t new_len,
                                                      unsigned char* old_data,
                                                      size_t old_len,
                                                      size_t patch_step_mem_size,
                                                      int is_use_big_cache_match,
                                                      size_t match_block_size,
                                                      size_t thread_num,
                                                      int min_single_match_score) {
    try {
        // The upstream API mutates its inputs, so copy them once up front.
        std::vector<unsigned char>* diff = new std::vector<unsigned char>();

        create_single_compressed_diff_block(
            new_data, new_data + new_len, old_data,
            old_data + old_len, *diff, nullptr, min_single_match_score,
            patch_step_mem_size, is_use_big_cache_match != 0, match_block_size, thread_num);

        diff->shrink_to_fit();
        return {diff->empty() ? nullptr : diff->data(), diff->size(), diff, 0};
    } catch (...) {
        return {nullptr, 0, nullptr, -1};
    }
}

HdiffBuffer hdiff_patch_single_compressed_diff(const unsigned char* old_data,
                                               size_t old_len,
                                               const unsigned char* diff_data,
                                               size_t diff_len) {
    try {
        hpatch_singleCompressedDiffInfo info;
        if (!getSingleCompressedDiffInfo_mem(&info, diff_data, diff_data + diff_len)) {
            return {nullptr, 0, nullptr, -1};
        }

        std::vector<unsigned char>* out_new =
            new std::vector<unsigned char>(static_cast<size_t>(info.newDataSize));

        // prepare streams
        hpatch_TStreamInput old_stream;
        mem_as_hStreamInput(&old_stream, old_data, old_data + old_len);
        hpatch_TStreamInput diff_stream;
        mem_as_hStreamInput(&diff_stream, diff_data, diff_data + diff_len);
        hpatch_TStreamOutput out_stream;
        mem_as_hStreamOutput(&out_stream, out_new->data(), out_new->data() + out_new->size());

        size_t temp_cache_size = 0;
        unsigned char* temp_cache = getPatchMemCache(
            /*isLoadOldAll=*/true, kPatchCacheSize_default,
            static_cast<size_t>(info.stepMemSize), info.oldDataSize, &temp_cache_size);
        if (!temp_cache) {
            delete out_new;
            return {nullptr, 0, nullptr, -1};
        }

        hpatch_BOOL ok = patch_single_compressed_diff(
            &out_stream, &old_stream, &diff_stream, info.diffDataPos, info.uncompressedSize,
            info.compressedSize, nullptr, info.coverCount, (hpatch_size_t)info.stepMemSize,
            temp_cache, temp_cache + temp_cache_size, nullptr);

        std::free(temp_cache);

        if (!ok) {
            delete out_new;
            return {nullptr, 0, nullptr, -1};
        }

        out_new->shrink_to_fit();
        return {out_new->empty() ? nullptr : out_new->data(), out_new->size(), out_new, 0};
    } catch (...) {
        return {nullptr, 0, nullptr, -1};
    }
}

void hdiff_free_buffer(void* handle) {
    if (!handle) return;
    delete static_cast<std::vector<unsigned char>*>(handle);
}

static int _read_trampoline(const hpatch_TStreamInput* stream,
                            hpatch_StreamPos_t readFromPos,
                            unsigned char* out_data,
                            unsigned char* out_data_end) {
    const HdiffIOCallbacks* cbs = static_cast<const HdiffIOCallbacks*>(stream->streamImport);
    if (!cbs || !cbs->read_fn) return 0;
    return cbs->read_fn(cbs->read_user, readFromPos, out_data, out_data_end);
}

static int _write_trampoline(const hpatch_TStreamOutput* stream,
                             hpatch_StreamPos_t writeToPos,
                             const unsigned char* data,
                             const unsigned char* data_end) {
    const HdiffIOCallbacks* cbs = static_cast<const HdiffIOCallbacks*>(stream->streamImport);
    if (!cbs || !cbs->write_fn) return 0;
    return cbs->write_fn(cbs->write_user, writeToPos, data, data_end);
}

int hdiff_patch_single_compressed_diff_callbacks(const HdiffIOCallbacks* callbacks,
                                                 hpatch_StreamPos_t old_size,
                                                 const unsigned char* diff_data,
                                                 size_t diff_len) {
    if (!callbacks || !callbacks->read_fn || !callbacks->write_fn) return -1;
    try {
        hpatch_singleCompressedDiffInfo info;
        if (!getSingleCompressedDiffInfo_mem(&info, diff_data, diff_data + diff_len)) {
            return -1;
        }

        hpatch_TStreamInput diff_stream;
        mem_as_hStreamInput(&diff_stream, diff_data, diff_data + diff_len);

        hpatch_TStreamInput old_stream;
        old_stream.streamImport = (void*)callbacks;
        old_stream.read = _read_trampoline;
        old_stream.streamSize = old_size;
        old_stream._private_reserved = 0;

        hpatch_TStreamOutput out_stream;
        out_stream.streamImport = (void*)callbacks;
        out_stream.write = _write_trampoline;
        out_stream.streamSize = info.newDataSize;
        out_stream.read_writed = 0;

        size_t temp_cache_size = 0;
        unsigned char* temp_cache = getPatchMemCache(
            /*isLoadOldAll=*/false, kPatchCacheSize_default,
            static_cast<size_t>(info.stepMemSize), info.oldDataSize, &temp_cache_size);
        if (!temp_cache) return -1;

        hpatch_BOOL ok = patch_single_compressed_diff(
            &out_stream, &old_stream, &diff_stream, info.diffDataPos, info.uncompressedSize,
            info.compressedSize, nullptr, info.coverCount, (hpatch_size_t)info.stepMemSize,
            temp_cache, temp_cache + temp_cache_size, nullptr);

        std::free(temp_cache);
        return ok ? 0 : -1;
    } catch (...) {
        return -1;
    }
}

} // extern "C"
