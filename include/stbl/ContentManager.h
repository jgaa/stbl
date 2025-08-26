#pragma once

#include <memory>
#include <regex>

#include "stbl/Node.h"
#include "stbl/Article.h"
#include "stbl/Content.h"
#include "stbl/Page.h"
#include "stbl/Series.h"
#include "stbl/Options.h"
#include "stbl/logging.h"


namespace stbl {

struct RenderCtx {
    // The node we are about to render
    node_t current;
    size_t url_recuse_level = 0; // Relative to the sites root

    std::string GetRelativeUrl(const std::string url) const {
        static const std::regex url_pattern(R"(^https?:\/\/.*)");

        if (regex_match(url, url_pattern)) {
            return url;
        }

        std::stringstream out;
        out << getRelativePrefix() << url;
        return out.str();
    }

    std::string getRelativePrefix() const {
        std::stringstream out;
        for(size_t level = 0; level < url_recuse_level; ++level) {
            out << "../";
        }
        return out.str();
    }
};

class ContentManager
{
protected:
    ContentManager() = default;

public:
    virtual ~ContentManager() = default;


    /*! Generate the site based on the managers options */
    virtual void ProcessSite() = 0;
    // Create HTML to list *n* articles
    virtual std::string ListArticles(const RenderCtx& ctx, size_t num) = 0;
    static const Options& GetOptions();
    static ContentManager& Instance();

public:
    /*! Factory */
    static std::shared_ptr<ContentManager> Create(const Options& options);

protected:
    static Options options_;
    static ContentManager *self_;
};

}
