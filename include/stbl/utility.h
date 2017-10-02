

#include <iostream>
#include <string>

//#include <boost/utility/string_ref.hpp>
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
          bool createDirectoryIsMissing = false);


boost::property_tree::ptree
LoadProperties(const boost::filesystem::path& path);

std::string ToString(const std::wstring& str);

void CopyDirectory(const boost::filesystem::path& src,
                   const boost::filesystem::path& dst);

}
