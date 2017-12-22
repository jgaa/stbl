#include <memory>
#include <string>
#include <boost/filesystem.hpp>

namespace stbl {

class Sitemap {
public:

    struct Entry {
        std::string url;
        std::string updated;
        float priority = 0.5;
        std::string changefreq;
    };

    Sitemap() = default;
    virtual ~Sitemap() = default;

    virtual void Add(const Entry& entry) = 0;

    virtual void Write(const boost::filesystem::path& path) = 0;

    static std::unique_ptr<Sitemap> Create();
};

}
