
#include <fstream>
#include <streambuf>
#include <iomanip>
#include <ctime>
#include <iostream>
#include <codecvt>

#include <boost/property_tree/ptree.hpp>
#include <boost/property_tree/info_parser.hpp>
#include <boost/lexical_cast.hpp>
#include <boost/uuid/uuid.hpp>
#include <boost/uuid/uuid_io.hpp>
#include <boost/uuid/uuid_generators.hpp>


#include "stbl/utility.h"
#include "stbl/logging.h"

using namespace std;
using boost::string_ref;
namespace pt = boost::property_tree;
namespace fs = boost::filesystem;

namespace stbl {

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
        CreateDirectoryForFile(path);
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

void CreateDirectoryForFile(const boost::filesystem::path& path) {
    const auto directory = path.parent_path();
    if (!is_directory(directory)) {
        LOG_DEBUG << "Creating directory: " << directory;
        create_directories(directory);
    }
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

string ToStringAnsi(const time_t& when) {
    std::tm tm = *std::localtime(&when);
    return boost::lexical_cast<string>(put_time(&tm, "%F %R"));
}

time_t Roundup(time_t when, const int roundup) {
    const bool add = (when % roundup) != 0;
    when /= roundup;
    when *= roundup;
    if (add) {
        when += roundup;
    }
    return when;
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
        fs::path d = dst;
        d /= de.path().filename();
        LOG_TRACE << "Copying " << de.path() << " --> " << d;
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

void EatHeader(std::istream& in) {

    int separators = 0;

    while(in) {
        if ((in && in.get() == '-')
            && (in && (in.get() == '-'))
            && (in && (in.get() == '-'))) {
            ++separators;
        }

        while(in && (in.get() != '\n'))
            ;

        if (separators == 2) {
            return;
        }
    }

    throw runtime_error("Parse error: Failed to locate header section.");
}

std::string CreateUuid() {
    boost::uuids::uuid uuid = boost::uuids::random_generator()();
    return boost::uuids::to_string(uuid);
}


}
