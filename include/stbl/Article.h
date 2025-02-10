#pragma once

#include "stbl/stbl.h"
#include "stbl/Node.h"
#include "stbl/Content.h"

namespace stbl {
class Scanner;

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
    virtual void SetContent(content_t content) = 0;
    virtual authors_t GetAuthors() const = 0;
    virtual void SetAuthors(const authors_t& authors) = 0;
    virtual void UpdateSourceHeaders(Scanner& scanner,
                                     const Node::Metadata& meta) = 0;
    virtual void SetSeries(const serie_t& serie) = 0;
    virtual serie_t GetSeries() const  = 0;

    virtual void setUpdated(time_t updated) = 0;

    static std::shared_ptr<Article> Create();
};

}
