#pragma once

#include <string>
#include <vector>
#include <time.h>
#include <functional>
#include <iostream>
#include <memory>

namespace stbl {


/*! Interface for Blog post or a series of blog-posts
*/
class Node
{
public:
    enum class Type { ARTICLE, SERIES };

    struct Metadata {
        std::string uuid;
        std::wstring title;
        std::string abstract;
        std::wstring menu;
        std::string tmplte;
        std::string type;
        std::string banner;
        std::string banner_credits;
        std::string comments;
        int sitemap_priority = -1;
        std::string sitemap_changefreq;
        std::vector<std::wstring> tags;
        time_t updated = 0;
        time_t published = 0;
        time_t expires = 0;
        bool is_published = true;
        std::string article_path_part;
        std::string relative_url;
        bool have_uuid = false;
        bool have_published = false;
        bool have_updated = false;
        bool have_title = false;
        int part = 0;

        time_t latestDate() const noexcept {
            return std::max(updated, published);
        }
    };


    Node() = default;
    virtual ~Node() = default;
    virtual Type GetType() const = 0;

    virtual std::shared_ptr<Metadata> GetMetadata() const = 0;
    virtual void SetMetadata(const std::shared_ptr<Metadata>& metadata) = 0;
};

::std::ostream& operator << (::std::ostream& out, const ::stbl::Node::Type& value);
::std::ostream& operator << (::std::ostream& out, const ::stbl::Node& node);

} // namespace stbl
