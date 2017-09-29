#pragma once

#include "stbl/stbl.h"
#include "stbl/Node.h"
#include "stbl/Content.h"

namespace stbl {

class Article : public Node
{
public:
    struct Header : public Metadata
    {
        std::vector<std::string> authors;
    };

    using authors_t = std::vector<std::string>;

    Article() = default;
    virtual ~Article() = default;

    virtual std::shared_ptr<Content> GetContent() = 0;
    virtual void SetContent(std::shared_ptr<Content>& content) = 0;
    virtual authors_t GetAuthors() const = 0;
    virtual void SetAuthors(const authors_t& authors) = 0;

    static std::shared_ptr<Article> Create();
};

}
