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
        std::wstring unique_id;
        std::wstring title;
        std::wstring subject;
        std::string abstract;
        std::vector<std::wstring> tags;
        time_t updated = 0;
        time_t published = 0;
        time_t expires = 0;
        bool is_published = true;
        std::string article_path_part;
        std::string relative_url;
    };


    Node() = default;
    virtual ~Node() = default;
    virtual Type GetType() const = 0;

    virtual std::shared_ptr<Metadata> GetMetadata() const = 0;
    virtual void SetMetadata(const std::shared_ptr<Metadata>& metadata) = 0;
};

}
