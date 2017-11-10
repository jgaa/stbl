#include <memory>
#include <vector>
#include <boost/filesystem.hpp>

#include "stbl/Image.h"

namespace stbl {

class ImageMgr {
public:
    using widths_t = std::vector<int>;

    struct ImageInfo {
        // Relative path from the sites root
        std::string relative_path;

        // Size of the image
        Image::Size size;
    };

    using images_t = std::vector<ImageInfo>;

    /*! Image manager
     *
     * \param widths List of desired widths for banner-images.
     */
    ImageMgr() = default;

    virtual ~ImageMgr() = default;

    /*! Prepares a list of banner-images
     *
     * This function will create a set of smaller images, matching
     * the widths argument to its factory, unless they
     * already exists.
     *
     * The returned list consists of alternative images that can be
     * used, sorted by size, smallest first. The idea is to prepare
     * several variants of each image for responsive web sites.
     */
    virtual images_t Prepare(const boost::filesystem::path& image) = 0;

    static std::unique_ptr<ImageMgr> Create(const widths_t& widths,
                                            int quality);
};

}

