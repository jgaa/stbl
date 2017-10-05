
#include <fstream>
#include <streambuf>
#include <iomanip>
#include <ctime>
#include <iostream>
#include <codecvt>

#include <boost/property_tree/ptree.hpp>
#include <boost/property_tree/info_parser.hpp>
#include <boost/lexical_cast.hpp>

#include "stbl/utility.h"
#include "stbl/logging.h"

using namespace std;
using boost::string_ref;
namespace pt = boost::property_tree;
namespace fs = boost::filesystem;

namespace stbl {

// boost::string_ref Sf(boost::string_ref::const_iterator start,
//                      boost::string_ref::const_iterator end,
//                      bool trim) {
//     boost::string_ref sf = {start, static_cast<std::size_t>(end - start)};
//     if (trim) {
//         while (!sf.empty()
//             && ((sf.front() == '\n')
//                 || (sf.front() == '\t')
//                 || (sf.front() == ' '))) {
//             sf = {sf.data() + 1, sf.size() -1};
//         }
//         while (!sf.empty()
//             && ((sf.back() == '\n')
//                 || (sf.back() == '\t')
//                 || (sf.back() == ' '))) {
//             sf = {sf.data(), sf.size() -1};
//         }
//     }
//     return sf;
// }

string Load(const fs::path& path) {

    if (!is_regular(path)) {
        LOG_ERROR << "The file " << path << " need to exist!";
        throw runtime_error("I/O error - Missing required file.");
    }

    std::ifstream t(path.string());
    string str;

    t.seekg(0, std::ios::end);
    str.reserve(t.tellg());
    t.seekg(0, std::ios::beg);

    str.assign((std::istreambuf_iterator<char>(t)),
        std::istreambuf_iterator<char>());

    return str;
}

void Save(const fs::path& path,
          const std::string& data,
          bool createDirectoryIsMissing) {

    if (createDirectoryIsMissing) {
        const auto directory = path.parent_path();
        if (!is_directory(directory)) {
            LOG_DEBUG << "Creating directory: " << path;
            create_directories(directory);
        }
    }

    std::ofstream out(path.string());

    if (!out) {
        auto err = strerror(errno);
        LOG_ERROR << "IO error. Failed to open "
            << path << " for write: " << err;

        throw runtime_error("IO error");
    }

    out << data;
}

boost::property_tree::ptree
LoadProperties(const fs::path& path) {
    if (!is_regular(path)) {
        LOG_ERROR << "The file " << path << " need to exist!";
        throw runtime_error("I/O error - Missing required file.");
    }

    LOG_TRACE << "Loading properties" << path;
    pt::ptree tree;
    pt::read_info(path.string(), tree);
    return tree;
}

std::string ToString(const std::wstring& str) {
    wstring_convert<codecvt_utf8<wchar_t>> converter;
    return converter.to_bytes(str);
}

std::wstring ToWstring(const std::string& str) {
    wstring_convert<std::codecvt_utf8_utf16<wchar_t>> converter;
    return converter.from_bytes(str);
}


void CopyDirectory(const fs::path& src,
                   const fs::path& dst) {

    if (!is_directory(src)) {
        LOG_ERROR << "The dirrectory "
            << src << " need to exist in order to copy it!";
        throw runtime_error("I/O error - Missing required directory.");
    }

    if (!is_directory(dst)) {
        create_directories(dst);
    }

    for (const auto& de : fs::directory_iterator{src})
    {
//         const auto& path = de.path();
//         auto relativePathStr = path.string();
//         boost::replace_first(relativePathStr, sourceDir.string(), "");
//         fs::copy(path, dst / relativePathStr);

        fs::path d = dst;
        d /= de.path().filename();
        LOG_DEBUG << "Copying " << de.path() << " --> " << d;
        if (is_regular(de.path())) {
            fs::copy_file(de.path(), d, fs::copy_option::overwrite_if_exists);
        } else if (is_symlink(de.path())) {
            fs::copy_symlink(de.path(), d);
        } else if (is_directory(de.path())) {
            CopyDirectory(de.path(), d);
        }  else {
            LOG_WARN << "Skipping " << de.path()
                << " from directory copy. I don't know what it is...";
        }
    }


}


}
