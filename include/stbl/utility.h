

#include <iostream>
#include <string>

#include <boost/filesystem.hpp>
#include <boost/property_tree/ptree.hpp>

namespace stbl {

// // Utility functions
// boost::string_ref Sf(boost::string_ref::const_iterator start,
//                      boost::string_ref::const_iterator end,
//                      bool trim = false) ;

std::string Load(const boost::filesystem::path& path);
void Save(const boost::filesystem::path& path,
          const std::string& data,
          bool createDirectoryIsMissing = false,
          bool binary = false);
void CreateDirectory(const boost::filesystem::path& path);
void CreateDirectoryForFile(const boost::filesystem::path& path);

boost::property_tree::ptree
LoadProperties(const boost::filesystem::path& path);

std::string ToString(const std::wstring& str);
std::wstring ToWstring(const std::string& str);
std::string ToStringAnsi(const time_t& when);
time_t Roundup(time_t when, const int roundup);

void CopyDirectory(const boost::filesystem::path& src,
                   const boost::filesystem::path& dst);

void EatHeader(std::istream& in);

std::string CreateUuid();

}

