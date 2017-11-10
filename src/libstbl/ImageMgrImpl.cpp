

#include "stbl/stbl.h"
#include "stbl/ImageMgr.h"
#include "stbl/logging.h"
#include "stbl/utility.h"

using namespace std;
using namespace std::string_literals;

namespace stbl {

class ImageMgrImpl : public ImageMgr
{
public:
    ImageMgrImpl(const widths_t& widths, int quality)
    : widths_{widths}, quality_{quality}
    {
    }

    images_t Prepare(const boost::filesystem::path & path) override {
        images_t images;
        static const string scale_dir{"_scale_"};

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
                    images.push_back(move(ii));
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

            if (boost::filesystem::exists(dst)) {
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

        return images;
    }

private:
    const widths_t widths_;
    const int quality_;
};


std::unique_ptr<ImageMgr> ImageMgr::Create(const ImageMgr::widths_t& widths,
                                           int quality) {
    return make_unique<ImageMgrImpl>(widths, quality);
}

}
