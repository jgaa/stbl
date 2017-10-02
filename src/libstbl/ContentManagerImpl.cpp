
#include <assert.h>
#include <deque>
#include <iomanip>
#include <ctime>
#include <fstream>
#include <iostream>
#include <map>
#include <regex>

#include <boost/lexical_cast.hpp>
#include <boost/algorithm/string.hpp>
#include <boost/lexical_cast.hpp>

#include "stbl/Options.h"
#include "stbl/ContentManager.h"
#include "stbl/Scanner.h"
#include "stbl/Node.h"
#include "stbl/Series.h"
#include "stbl/logging.h"
#include "stbl/utility.h"

using namespace std;
using namespace boost::filesystem;
using namespace std::string_literals;

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
    : options_{options}, now_{time(nullptr)}
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
                case Node::Type::SERIES: {
                    auto s = dynamic_pointer_cast<Series>(n);
                    assert(s);
                    AddSeries(s);
                } break;
                case Node::Type::ARTICLE: {
                    auto a = dynamic_pointer_cast<Article>(n);
                    assert(a);
                    AddArticle(a);
                } break;
            }
        }
    }

    void MakeTempSite()
    {
        std::vector<string> directories_to_copy{
            "images", "artifacts"
        };

        // Create the main page from template
        RenderFrontpage();

        // Create an overview page with all published articles in a tree.

        // Create XSS feed pages.
        //    - One global
        //    - One for each subject

        // Render the series and articles
        for(auto& ai : all_articles_) {
            RenderArticle(*ai);
        }

        for(auto& n : all_series_) {
            RenderSerie(*n);
        }

        // Copy artifacts, images and other files
        for(const auto& d : directories_to_copy) {
            path src = options_.source_path, dst = tmp_path_;
            src /= d;
            dst /= d;
            CopyDirectory(src, dst);
        }
    }

    void RenderArticle(const ArticleInfo& ai) {
        // TODO: Handle multiple pages
        for(auto& p : ai.article->GetContent()->GetPages()) {

            LOG_DEBUG << "Generating " << *ai.article
                << " --> " << ai.tmp_path;

            const auto directory = ai.tmp_path.parent_path();
            if (!is_directory(directory)) {
                create_directories(directory);
            }

            stringstream content;
            p->Render2Html(content);

            string article = LoadTemplate("article.html");
            map<string, string> vars;
            AssignDefauls(vars);
            auto meta = ai.article->GetMetadata();
            Assign(*meta, vars);
            AssignHeaderAndFooter(vars);
            vars["content"] = content.str();
            ProcessTemplate(article, vars);
            Save(ai.tmp_path, article, true);
        }
    }

    void RenderSerie(Series& serie) {
        string series = LoadTemplate("series.html");

        const auto meta = serie.GetMetadata();
        path dst = tmp_path_;
        dst /= meta->relative_url;

        LOG_TRACE << "Generating " << serie << " --> " << dst;


        std::map<std::string, std::string> vars;
        vars["article-type"] = boost::lexical_cast<string>(serie.GetType());
        AssignDefauls(vars);
        AssignHeaderAndFooter(vars);
        Assign(*meta, vars);

        auto articles = serie.GetArticles();
        vars["list-articles"] = RenderNodeList(articles, true);

        ProcessTemplate(series, vars);
        Save(dst, series, true);
    }

    void AssignDefauls(map<string, string>& vars) {
        vars["now"] = ToStringLocal(now_);
        vars["now-ansi"] = ToStringAnsi(now_);
        vars["site-title"] = options_.options.get<string>("name", "Anonymous Nest");
        vars["site-abstract"] = options_.options.get<string>("abstract");
        vars["site-url"] = options_.options.get<string>(
            "url", options_.destination_path + "index.html");
    }

    void Assign(const Node::Metadata& md, map<string, string>& vars) {
        vars["updated"] = ToStringLocal(md.updated);
        vars["published"] = ToStringLocal(md.published);
        vars["expires"] = ToStringLocal(md.expires);
        vars["updated-ansi"] = ToStringAnsi(md.updated);
        vars["published-ansi"] = ToStringAnsi(md.published);
        vars["expires-ansi"] = ToStringAnsi(md.expires);
        vars["title"] = stbl::ToString(md.title);
        vars["abstract"] = md.abstract;
        vars["url"] = md.relative_url;
    }

    void CommitToDestination()
    {
        // TODO: Copy only files that have changed.
        // Make checksums for all the files in the tmp site.
        // Make checksums of the files in the destination site.

        CopyDirectory(tmp_path_, options_.destination_path);
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

    bool AddSeries(const serie_t& node) {
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

        auto meta = node->GetMetadata();
        meta->relative_url = meta->article_path_part + "/index.html";

        articles_for_frontpages_.push_back(node);
        all_series_.push_back(node);

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

        meta->relative_url = ai->relative_url;

        LOG_TRACE << *article << " has destinations:";
        LOG_TRACE    << "  relative_url: " << ai->relative_url;
        LOG_TRACE    << "  dst_path    : " << ai->dst_path;
        LOG_TRACE    << "  tmp_path    : " << ai->tmp_path;

        all_articles_.push_back(ai);
    }

    void RenderFrontpage() {
        string frontpage = LoadTemplate("frontpage.html");

        std::map<std::string, std::string> vars;

        AssignDefauls(vars);

        vars["now-ansi"] = ToStringAnsi(now_);
        vars["title"] = vars["site-title"];
        vars["abstract"] = vars["site-abstract"];
        vars["url"] = vars["site-url"];

        AssignHeaderAndFooter(vars);

        vars["list-articles"] = RenderNodeList(articles_for_frontpages_);

        ProcessTemplate(frontpage, vars);

        path frontpage_path = tmp_path_;
        frontpage_path /= "index.html";
        Save(frontpage_path, frontpage);
    }

    void AssignHeaderAndFooter(std::map<std::string, std::string>& vars) {
        string page_header = LoadTemplate("page-header.html");
        string site_header = LoadTemplate("site-header.html");
        string footer = LoadTemplate("footer.html");
        ProcessTemplate(page_header, vars);
        ProcessTemplate(site_header, vars);
        ProcessTemplate(footer, vars);
        vars["page-header"] = page_header;
        vars["site-header"] = site_header;
        vars["footer"] = footer;
    }

    template <typename NodeListT>
    string RenderNodeList(const NodeListT& nodes,
                          bool stripSeriesDir = false) {
        std::stringstream out;

        for(const auto& n : nodes) {
            map<string, string> vars;
            AssignDefauls(vars);
            const auto meta = n->GetMetadata();
            vars["article-type"] = boost::lexical_cast<string>(n->GetType());
            Assign(*meta, vars);

            if (stripSeriesDir) {
                const string url = meta->relative_url;
                auto pos = url.find('/');
                if (pos != url.npos) {
                    vars["url"] = url.substr(pos + 1);
                }
            }

            string item = LoadTemplate("article-in-list.html");
            ProcessTemplate(item, vars);
            out << item << endl;
        }

        return out.str();
    }

    string ToStringAnsi(const time_t& when) {
        std::tm tm = *std::localtime(&when);
        return boost::lexical_cast<string>(put_time(&tm, "%F %R"));
    }

    string ToStringLocal(const time_t& when) {
        std::tm tm = *std::localtime(&when);
        return boost::lexical_cast<string>(put_time(&tm, "%c"));
    }

    void ProcessTemplate(string& tmplte,
                         const std::map<std::string, std::string>& vars ) {

        // Expand all the macros we know about
        for (const auto& macro : vars) {
            const std::string name = "{{"s + macro.first + "}}"s;

            boost::replace_all(tmplte, name, macro.second);
        }

        // Remove other macros
        string result;
        result.reserve(tmplte.size());
        static const regex macro_pattern(R"(\{\{[\w\-]+\}\})");
        regex_replace(back_inserter(result), tmplte.begin(), tmplte.end(),
                      macro_pattern, "");

        tmplte = result;
    }

    string LoadTemplate(string name) const {
        path template_path = options_.source_path;
        template_path /= "templates";
        template_path /= name;

        return Load(template_path);
    }

    Options options_;

    // All the nodes, including expired and not published ones
    nodes_t nodes_;

    // All articles that are published and not expired
    deque<shared_ptr<ArticleInfo>> all_articles_;
    deque<std::shared_ptr<Series>> all_series_;

    // All articles and series that are to be listed on the front-page(s)
    deque<node_t> articles_for_frontpages_;

    path tmp_path_;

    const time_t now_;
};

std::shared_ptr<ContentManager> ContentManager::Create(const Options& options)
{
    return make_shared<ContentManagerImpl>(options);
}

}

