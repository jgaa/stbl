
#include <assert.h>
#include <deque>
#include <iomanip>
#include <ctime>
#include <fstream>
#include <iostream>

#include <boost/lexical_cast.hpp>

#include "stbl/Options.h"
#include "stbl/ContentManager.h"
#include "stbl/Scanner.h"
#include "stbl/Node.h"
#include "stbl/Series.h"
#include "stbl/logging.h"

using namespace std;
using namespace boost::filesystem;

namespace stbl {

class ContentManagerImpl : public ContentManager
{
public:
    struct ArticleInfo {
        article_t article;
        string relative_url; // Relative to the websites root
        path tmp_path;
        path dst_path;
    };

    ContentManagerImpl(const Options& options)
    : options_{options}
    {
    }

    ~ContentManagerImpl() {
        CleanUp();
    }

    void ProcessSite() override
    {
        Scan();
        Prepare();
        MakeTempSite();
        CommitToDestination();
    }


protected:
    void Scan()
    {
        auto scanner = Scanner::Create(options_);
        nodes_= scanner->Scan();

        LOG_DEBUG << "Listing nodes after scan: ";
        for(const auto& n: nodes_) {
            LOG_DEBUG << "  " << *n;

            if (n->GetType() == Node::Type::SERIES) {
                const auto& series = dynamic_cast<const Series&>(*n);
                for(const auto& a : series.GetArticles()) {
                    LOG_DEBUG << "    ---> " << *a;
                }
            }
        }
    }

    void Prepare()
    {
        tmp_path_ = temp_directory_path();
        tmp_path_ /= unique_path();

        create_directories(tmp_path_);

        // Go over tags.
        //    - Create a list of all tags
        //    - Add reference to article
        //    - Sort the list

        // Go over subjects
        //    - Create a list of all subjects
        //    - Add reference to article
        //    - Sort the list

        for(const auto& n: nodes_) {
            switch(n->GetType()) {
                case Node::Type::SERIES:
                    AddSeries(n);
                    break;
                case Node::Type::ARTICLE:
                {
                    auto a = dynamic_pointer_cast<Article>(n);
                    assert(a);
                    AddArticle(a);
                    break;
                }
            }
        }
    }

    void MakeTempSite()
    {
        // Create the main page from template

        // Create an overview page with all published articles in a tree.

        // Create XSS feed pages.
        //    - One global
        //    - One for each subject

        // Render the series and articles
        for(auto& ai : all_articles_) {
            // TODO: Handle multiple pages
            for(auto& p : ai->article->GetContent()->GetPages()) {

                LOG_DEBUG << "Generating " << * ai->article
                    << " at : " << ai->tmp_path;

                const auto directory = ai->tmp_path.parent_path();
                if (!is_directory(directory)) {
                    create_directories(directory);
                }

                std::ofstream out(ai->tmp_path.string());

                if (!out) {
                    auto err = strerror(errno);
                    LOG_ERROR << "IO error. Failed to open "
                        << ai->tmp_path << " for write: " << err;

                    throw runtime_error("IO error");
                }

                p->Render2Html(out);
            }
        }

        // Copy artifacts, images and other files
    }

    void CommitToDestination()
    {
        // Make checksums for all the files in the tmp site.
        // Make checksums of the files in the destination site.
        // Copy all files that have changed.
    }

    void CleanUp()
    {
        // Remove the temp site
        if (!options_.keep_tmp_dir && !tmp_path_.empty() && is_directory(tmp_path_)) {
            LOG_DEBUG << "Removing temporary directory " << tmp_path_;
            remove_all(tmp_path_);
        }

        // Remove any other temporary files
    }

    bool Validate(const node_t& node) {
        const auto meta = node->GetMetadata();
        const auto now = time(NULL);

        LOG_TRACE << "Evaluating " << *node << " for publishing...";

        if (!meta->is_published) {
            LOG_INFO << *node << " is held back because it is unpublished.";
            return false;
        }

        if (meta->published > now) {
            LOG_INFO << *node
                << " is held back because it is due to be published at "
                << put_time(localtime(&meta->published), "%Y-%m-%d %H:%M");
            return false;
        }

        if (meta->expires && (meta->expires < now)) {
            LOG_INFO << *node
                << " is held back because it expired at "
                << put_time(localtime(&meta->expires), "%Y-%m-%d %H:%M");
            return false;
        }

        return true;
    }

    bool AddSeries(const node_t& node) {
        if (!Validate(node)) {
            return false;
        }

        articles_t publishable;

        auto series = dynamic_pointer_cast<Series>(node);

        for(const auto& a : series->GetArticles()) {
            if (!Validate(a)) {
                continue;
            }

            publishable.push_back(a);
        }

        if (publishable.empty()) {
            LOG_INFO << *node
                << " is held back because it has no published articles";
            return false;
        }

        for(const auto& a : publishable) {
            DoAddArticle(a, series);
        }

        articles_for_frontpages_.push_back(node);

        return true;
    }

    bool AddArticle(const article_t& article) {
        if (!Validate(article)) {
            return false;
        }

        DoAddArticle(article);
        articles_for_frontpages_.push_back(article);

        return true;
    }

    void DoAddArticle(const article_t& article, serie_t series = {}) {
        static const string file_extension{".html"};
        auto ai = make_shared<ArticleInfo>();
        auto meta = article->GetMetadata();

        ai->article = article;

        ai->dst_path = options_.destination_path;
        ai->tmp_path = tmp_path_;

        string base_path;
        if (series) {
            string article_path;

            if (options_.path_layout == Options::PathLayout::SIMPLE) {
                article_path = series->GetMetadata()->article_path_part;
                base_path = article_path + "/";
            }
            ai->dst_path /= article_path;
            ai->tmp_path /= article_path;
        }

        const auto file_name =  meta->article_path_part + file_extension;
        ai->relative_url = base_path + file_name;
        ai->dst_path /= file_name;
        ai->tmp_path /= file_name;

        LOG_TRACE << *article << " has destinations:";
        LOG_TRACE    << "  relative_url: " << ai->relative_url;
        LOG_TRACE    << "  dst_path    : " << ai->dst_path;
        LOG_TRACE    << "  tmp_path    : " << ai->tmp_path;

        all_articles_.push_back(ai);
    }

    Options options_;

    // All the nodes, including expired and not published ones
    nodes_t nodes_;

    // All articles that are published and not expired
    deque<shared_ptr<ArticleInfo>> all_articles_;

    // All articles and series that are to be listed on the frontpage(s)
    deque<node_t> articles_for_frontpages_;

    path tmp_path_;
};

std::shared_ptr<ContentManager> ContentManager::Create(const Options& options)
{
    return make_shared<ContentManagerImpl>(options);
}

}

