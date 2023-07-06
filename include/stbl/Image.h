#include <memory>
#include <filesystem>

namespace stbl {

class Image {
public:
    struct Size {
        int width = 0;
        int height = 0;
    };

    Image() = default;
    virtual ~Image() = default;
    virtual Size ScaleAndSave(const std::filesystem::path& path,
                              int width,
                              int quality = 95) = 0;
    virtual int GetWidth() const = 0;
    virtual int GetHeight() const = 0;

    static std::unique_ptr<Image> Create(const std::filesystem::path& path);
};

}
