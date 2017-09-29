
#include <memory>
#include "stbl/Article.h"

using namespace std;

namespace stbl {


class ArticleImpl : public Article
{
public:
    ArticleImpl()
    {
    }

    ~ArticleImpl()  {
    }

    stbl::Node::Type GetType() override {
        return Type::SERIES;
    }

    std::shared_ptr<Metadata> GetMetadata() override {
        return metadata_;
    }

    void SetMetadata(const std::shared_ptr<Metadata> & metadata) override
    {
        metadata_ = metadata;
    }

    authors_t GetAuthors() const override {
        return authors_;
    }

    void SetAuthors(const authors_t & authors) override {
        authors_ = authors;
    }

    std::shared_ptr<Content> GetContent() override {
        return content_;
    }

    void SetContent(std::shared_ptr<Content> & content) override {
        content_ = content;
    }

private:
    articles_t articles_;
    std::shared_ptr<Metadata> metadata_;
    authors_t authors_;
    std::shared_ptr<Content> content_;
};

std::shared_ptr<Article> Article::Create() {
    return make_shared<ArticleImpl>();
}

}

