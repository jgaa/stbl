#pragma once

#include "stbl/stbl.h"
#include "stbl/Node.h"

namespace stbl {

class Series : public Node
{
public:
    Series() = default;
    virtual ~Series() = default;

    virtual articles_t GetArticles() const = 0;
    virtual void AddArticle(std::shared_ptr<Article>& article) = 0;
    virtual void AddArticles(articles_t articles) = 0;

    static std::shared_ptr<Series> Create();
};

}
