#pragma once

#include <string>

#include "stbl/Article.h"

namespace stbl {

class HeaderParser
{
public:
    using header_map_t = std::map<std::string, std::string>;

    HeaderParser() = default;
    virtual ~HeaderParser() = default;

    virtual void Parse(Article::Header& header, std::string& headerSection) = 0;

    static std::unique_ptr<HeaderParser> Create();
};

}
