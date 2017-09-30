#pragma once

#include <ostream>

#include <boost/filesystem.hpp>

#include "stbl/stbl.h"

namespace stbl {

class Page
{
protected:
    Page() = default;
    Page(const Page&) = delete;
    Page(Page&&) = delete;
    Page& operator = (const Page&) = delete;
    Page& operator = (Page&&) = delete;

public:
    virtual ~Page() = default;

    virtual void Render2Html(std::ostream& out) = 0;

    static page_t Create(const boost::filesystem::path& path);
};

}

