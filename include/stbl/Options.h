
#pragma once
#include <string>

namespace stbl {

struct Options
{
    enum PathLayout {
        SIMPLE, // Single articles in root, series in folders
        RECURSIVE // Tree structure
    };

    std::string source_path;
    std::string destination_path;
    PathLayout path_layout = PathLayout::SIMPLE;
};

}
