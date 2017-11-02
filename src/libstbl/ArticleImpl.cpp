
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

    stbl::Node::Type GetType() const override {
        return Type::ARTICLE;
    }

    std::shared_ptr<Metadata> GetMetadata() const override {
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

    void SetContent(content_t content) override {
        content_ = move(content);
    }

    void UpdateSourceHeaders(stbl::Scanner & scanner,
                             const Node::Metadata& meta) override {
        if (content_) {
            content_->UpdateSourceHeaders(scanner, meta);
        }
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

