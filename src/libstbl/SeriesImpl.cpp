
#include <memory>
#include "stbl/Series.h"

using namespace std;

namespace stbl {


class SeriesImpl : public Series
{
public:
    SeriesImpl() = default;

    virtual ~SeriesImpl() = default;

    void AddArticle(std::shared_ptr<Article> & article) override {
        articles_.push_back(article);
    }

    void AddArticles(articles_t articles) override {
        articles_.insert(articles_.end(), articles.begin(), articles.end());
    }

    stbl::Node::Type GetType() const override {
        return Type::SERIES;
    }

    articles_t GetArticles() const override {
        return articles_;
    }

    std::shared_ptr<Metadata> GetMetadata() const override {
        return metadata_;
    }

    void SetMetadata(const std::shared_ptr<Metadata> & metadata) override
    {
        metadata_ = metadata;
    }

private:
    articles_t articles_;
    std::shared_ptr<Metadata> metadata_;
};

std::shared_ptr<Series> Series::Create() {
    return make_shared<SeriesImpl>();
}

}
