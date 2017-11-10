

#include <boost/gil/image.hpp>
#include <boost/gil/typedefs.hpp>
#include <boost/gil/extension/io/jpeg_io.hpp>
#include <boost/gil/extension/numeric/sampler.hpp>
#include <boost/gil/extension/numeric/resample.hpp>

#include "stbl/stbl.h"
#include "stbl/Image.h"
#include "stbl/logging.h"

using namespace std;
using namespace boost::gil;

// Using boost::gil for image processing for now. It takes /forever/ to
// compile, and the numeric extension is not yet in the boost release branch
// (as of debian stretch) - but I want to have as few external dependencies as
// possible.

namespace stbl {

class ImageImpl : public Image {
public:

    ImageImpl(const boost::filesystem::path& path)
    : path_{path}
    {
        jpeg_read_image(path.c_str(), img_);
    }

    Size ScaleAndSave(const boost::filesystem::path& path,
                      int width,
                      int quality) override {
        const auto w = img_.width();
        const auto h = img_.height();
        double rw = static_cast<double>(w) / static_cast<double>(width);
        const int height = static_cast<int>(static_cast<double>(h) / rw);
        rgb8_image_t area(width, height);
        LOG_TRACE << "Scaling image " << path_
            << " from " << w << 'x' << h
            << " to " << width << 'x' << height
            << " in " << path;
        resize_view(const_view(img_), view(area), bilinear_sampler());
        jpeg_write_view(path.c_str(), const_view(area), quality);

        Size s;
        s.width = area.width();
        s.height = area.height();
        return s;
    }

    int GetWidth() const override {
        return img_.width();
    }

    int GetHeight() const override {
        return img_.height();
    }

private:
    rgb8_image_t img_;
    const boost::filesystem::path path_;
};

unique_ptr<Image> Image::Create(const boost::filesystem::path& path) {
    return make_unique<ImageImpl>(path);
}

}
