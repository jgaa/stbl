#pragma once

#include <string>
#include <vector>
#include <time.h>

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
        std::vector<std::wstring> tags;
        time_t updated = 0;
        time_t published = 0;
        time_t expires = 0;
    };


    Node() = default;
    virtual ~Node() = default;
    virtual Type GetType() = 0;

    virtual std::shared_ptr<Metadata> GetMetadata() = 0;
    virtual void SetMetadata(const std::shared_ptr<Metadata>& metadata) = 0;
};

}
