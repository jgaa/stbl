#include <memory>
#include <set>
#include <fstream>
#include <streambuf>
#include <iomanip>
#include <filesystem>

#include "stbl/Sitemap.h"
#include "stbl/logging.h"
#include "stbl/utility.h"

using namespace std;

namespace stbl {

class SitemapImpl : public Sitemap {
public:
    struct Cmp {
        bool operator()(const Entry& left, const Entry& right) const {
            return left.url < right.url;
        }
    };

    void Add(const stbl::Sitemap::Entry & entry) override {
        if (entry.url.empty()) {
            return;
        }
        entries_.insert(entry);
    }

    void Write(const std::filesystem::path & path) override {

        LOG_TRACE << "Saving sitemap: " << path;

        CreateDirectoryForFile(path);

        std::ofstream out(path.string(), ios_base::out | ios_base::trunc);

        if (!out) {
            auto err = strerror(errno);
            LOG_ERROR << "IO error. Failed to open "
                << path << " for write: " << err;

            throw runtime_error("IO error");
        }

        out << R"(<?xml version="1.0" encoding="UTF-8"?>)" << endl
            << R"(<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">)"
            << endl;

        for(const auto& e: entries_) {

            auto date = e.updated;
            date.resize(10); // we want only the date

            out << "  <url>" << endl
                << "    <loc>" << e.url << "</loc>" << endl
                << "    <lastmod>" << date << "</lastmod>" << endl
                << "    <priority>" << e.priority<< "</priority>" << endl;

            if (!e.changefreq.empty()) {
                out << "    <changefreq>" << e.changefreq << "</changefreq>" << endl;
            }

            out << "  </url>" << endl;
        }

        out << "</urlset>" << endl;
    }

private:
    std::set<Entry, Cmp> entries_;
};

std::unique_ptr<Sitemap> Sitemap::Create() {
    return make_unique<SitemapImpl>();
}

}
