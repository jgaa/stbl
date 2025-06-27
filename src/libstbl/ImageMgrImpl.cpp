
#include "stbl/stbl.h"
#include "stbl/ImageMgr.h"
#include "stbl/logging.h"
#include "stbl/utility.h"

using namespace std;
using namespace std::string_literals;

namespace stbl {

namespace {
    bool imageExists(const std::filesystem::path& path, const filesystem::file_time_type& orig_time) {
        if (std::filesystem::exists(path)) {
            // Compare write times
            const auto last_write_time = std::filesystem::last_write_time(path);
            if (last_write_time >= orig_time) {
                LOG_TRACE << "The image " << path << " already exists.";
                return true;
            }
        }
        return false;
    }
}

class ImageMgrImpl : public ImageMgr
{
public:
    ImageMgrImpl(const widths_t& widths, int quality)
    : widths_{widths}, quality_{quality}
    {
    }

    images_t Prepare(const std::filesystem::path & path) override {
        images_t images;
        static const string scale_dir{"_scale_"};

        if (!std::filesystem::exists(path)) {
            LOG_ERROR << "Image does not exist: " << path;
            return images; // Return empty if the image does not exist
        }

        const auto updated_time = std::filesystem::last_write_time(path);

        auto image = Image::Create(path);
        int largest_width = 0;

        for (auto w = widths_.cbegin(); w != widths_.cend(); ++w) {
            if (*w >= image->GetWidth()) {
                if (largest_width < image->GetWidth()) {
                    // Use the original image
                    ImageInfo ii;
                    ii.relative_path = "images/"s + path.filename().string();
                    ii.size.width = image->GetWidth();
                    ii.size.height = image->GetHeight();
                    images.push_back(std::move(ii));
                }
                break;
            }

            auto dst = path.parent_path();
            dst /= scale_dir + to_string(*w);
            dst /= path.filename();

            largest_width = *w;

            ImageInfo ii;
            ii.relative_path = "images/"s
                + scale_dir + to_string(*w)
                + "/"s + path.filename().string();

            if (imageExists(dst, updated_time)) {
                LOG_TRACE << "The scaled image " << dst << " already exists.";
                auto scaled_img = Image::Create(dst);
                ii.size.width = scaled_img->GetWidth();
                ii.size.height = scaled_img->GetHeight();
            } else {
                CreateDirectoryForFile(dst);
                ii.size = image->ScaleAndSave(dst, *w, quality_);
            }

            images.push_back(move(ii));
        }

        // Sort, largest first
        std::sort(images.begin(), images.end() ,
                  [](const ImageInfo& a, const ImageInfo& b) {
                      return a.size.width > b.size.width;
        });

        return images;
    }

private:
    const widths_t widths_;
    const int quality_;
};


std::unique_ptr<ImageMgr> ImageMgr::Create(const ImageMgr::widths_t& widths, int quality) {
    return make_unique<ImageMgrImpl>(widths, quality);
}

}
