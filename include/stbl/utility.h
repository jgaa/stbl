#pragma once

#include <iostream>
#include <string>

#include <filesystem>
#include <boost/property_tree/ptree.hpp>

namespace stbl {

// // Utility functions
// boost::string_ref Sf(boost::string_ref::const_iterator start,
//                      boost::string_ref::const_iterator end,
//                      bool trim = false) ;

std::string Load(const std::filesystem::path& path);
void Save(const std::filesystem::path& path,
          const std::string& data,
          bool createDirectoryIsMissing = false,
          bool binary = false);
void CreateDirectory(const std::filesystem::path& path);
void CreateDirectoryForFile(const std::filesystem::path& path);

boost::property_tree::ptree
LoadProperties(const std::filesystem::path& path);

std::string ToString(const std::wstring& str);
std::wstring ToWstring(const std::string& str);
std::string ToStringAnsi(const time_t& when);
time_t Roundup(time_t when, const int roundup);

void CopyDirectory(const std::filesystem::path& src,
                   const std::filesystem::path& dst);

void EatHeader(std::istream& in);

std::string CreateUuid();

std::filesystem::path MkTmpPath();

template <typename T>
auto escapeForXml(const T& orig) {
    std::ostringstream out;
    for(const auto ch : orig) {
        if (ch == '<') {
            out << "&lt;";
        } else if (ch == '>') {
            out << "&gt;";
        } else if (ch == '\"') {
            out << "&quot;";
        } else if (ch == '\'') {
            out << "&apos;";
        } else {
            out << ch;
        }
    }
    return out.str();
}

std::string Pipe(const std::string& cmd,
                 const std::vector<std::string>& args,
                 const std::string& input);

}
