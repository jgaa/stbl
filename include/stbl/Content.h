#pragma once

#include <boost/filesystem.hpp>

#include "stbl/stbl.h"
#include "stbl/Node.h"

namespace stbl {

class Scanner;

class Content
{
public:
    Content() = default;
    virtual ~Content() = default;

    virtual void AddPage(page_t page) = 0;
    virtual pages_t GetPages() = 0;
    // Update page headers if needed.
    virtual void UpdateSourceHeaders(Scanner& scanner,
                                     const Node::Metadata& meta) = 0;

    static content_t Create(const boost::filesystem::path& path);
};

}
