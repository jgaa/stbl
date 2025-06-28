#pragma once

#include <ostream>
#include <filesystem>

#include <boost/asio/awaitable.hpp>

#include "stbl/stbl.h"

namespace stbl {

class Scanner;
class RenderCtx;

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

    // Return the number of words in the article
    virtual boost::asio::awaitable<size_t> Render2Html(std::ostream& out, RenderCtx& ctx) = 0;
    static page_t Create(const std::filesystem::path& path);
    static page_t Create(const std::string& content);
    virtual bool containsVideo() const noexcept { return false; }
    virtual std::string getVideOptions() const {return {};}
};

}

