#pragma once
#include <string>
#include <boost/property_tree/ptree.hpp>

namespace stbl {

struct Options
{
    enum PathLayout {
        SIMPLE, // Single articles in root, series in folders
        RECURSIVE // Tree structure
    };

    // From command-line
    std::string source_path;
    std::string destination_path;
    PathLayout path_layout = PathLayout::SIMPLE;
    bool keep_tmp_dir = false;
    std::string open_in_browser;
    bool publish = false; // Require 'publish.command' to be set in config
    std::string publish_destination;

    // From stbl.conf
    boost::property_tree::ptree options;
};

}
