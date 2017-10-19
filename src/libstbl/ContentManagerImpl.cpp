
#include <assert.h>
#include <deque>
#include <iomanip>
#include <ctime>
#include <fstream>
#include <iostream>
#include <map>
#include <regex>
#include <algorithm>
#include <set>

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

    struct TagInfo {
        nodes_t nodes;
        string name; //utf8 with caps as in first seen version
        string url;
    };

    struct RenderCtx {
        // The node we are about to render
        node_t current;
        size_t url_recuse_level = 0; // Relative to the sites root

        string GetRelativeUrl(const string url) const {
            stringstream out;
            for(size_t level = 0; level < url_recuse_level; ++level) {
                out << "../";
            }
            out << url;
            return out.str();
        }
    };

    struct Menu {
        wstring name;
        string url;
        vector<shared_ptr<Menu>> children;
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
        if (options_.publish) {
            Publish();
        }
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
        // Prepare menus from config
        ScanMenus(menu_, options_.options.get_child("menu"));

        tmp_path_ = temp_directory_path();
        tmp_path_ /= unique_path();
        create_directories(tmp_path_);

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

        // Prepare tags
        for(auto& tag : tags_) {
            auto path =  stbl::ToString(tag.first);
            boost::replace_all(path, " ", "_");
            tag.second.url = "_tags/"s + path + ".html";
        }
    }

    template <typename T>
    void ScanMenus(Menu& parent, const T& mlist) {
        for(const auto& n : mlist) {
            auto menu = make_shared<Menu>();
            menu->name= stbl::ToWstring(n.first);
            menu->url = n.second.get("", "");
            if (menu->url.empty()) {
                ScanMenus(*menu, n.second);
            }
            parent.children.push_back(menu);
            LOG_TRACE <<  "Adding menu << " << stbl::ToString(parent.name)
                << "/" << stbl::ToString(menu->name)
                << " --> " << menu->url;
        }
    }

    // Add or merge a menu at any level into the menu-tree
    void AddToMenu(const wstring& name, string url) {

        LOG_TRACE << "Adding menu-item: \"" << stbl::ToString(name)
            << "\" --> " << url;

        vector<wstring> parts;
        boost::split(parts, name, boost::is_any_of("/"));
        Menu *current_menu = &menu_;

        int depth = 0;
        for(const auto& p: parts) {

            // Recurse the existing menu structure
            bool match = false;
            for(auto& m : current_menu->children) {
                if (m->name == p) {
                    ++depth;
                    current_menu = m.get();
                    match = true;
                    break;
                }
            }

            if (match && depth < parts.size()) {
                continue;
            }

            // Just update the existing node
            if (match) {
                if (!current_menu->url.empty()) {
                    LOG_WARN << "Overriding existing menu \"" << name
                        << "\": " << current_menu->url << " --> " << url;
                }

                current_menu->url = url;
                return;
            }

            // When we get here, we must add new node(s) to the menu.
            do {
                auto new_menu = make_shared<Menu>();
                new_menu->name = parts[depth];
                if (++depth == parts.size()) {
                    new_menu->url = url;
                }

                current_menu->children.push_back(move(new_menu));
                current_menu = current_menu->children.back().get();

            } while(depth < parts.size());

            return; // never continue the outer loop at this point
        }
    }

    void MakeTempSite()
    {
        std::vector<string> directories_to_copy{
            "images", "artifacts", "files"
        };

        // Create the main page from template
        RenderFrontpage();

        // Create an overview page with all published articles in a tree.

        // Create XSS feed pages.
        //    - One global
        //    - One for each subject

        // Render the articles
        for(auto& ai : all_articles_) {
            RenderArticle(*ai);
        }

        // Render the series
        for(auto& n : all_series_) {
            RenderSerie(n);
        }

        // Render tags
        for(auto& t: tags_) {
            RenderTag(t.second);
        }

        // Copy artifacts, images and other files
        for(const auto& d : directories_to_copy) {
            path src = options_.source_path, dst = tmp_path_;
            src /= d;
            dst /= d;
            if (boost::filesystem::is_directory(src)) {
                CopyDirectory(src, dst);
            } else {
                LOG_WARN << "Cannot copy directory " << src
                    << ", it does not exist.";
            }
        }
    }

    void RenderTag(const TagInfo& ti) {
        if (ti.nodes.empty()) {
            // Not used
            LOG_TRACE << "Ignoring unused tag.";
            return;
        }

        RenderCtx ctx;
        ctx.url_recuse_level = GetRecurseLevel(ti.url);

        auto page = LoadTemplate("tags.html");

        map<string, string> vars;
        AssignDefauls(vars, ctx);
        vars["name"] = ti.name;
        vars["title"] = ti.name;
        vars["url"] = ctx.GetRelativeUrl(ti.url);
        AssignHeaderAndFooter(vars, ctx);
        vars["list-articles"] = RenderNodeList(ti.nodes, ctx);
        ProcessTemplate(page, vars);

        path dest = tmp_path_;
        dest /= ti.url;
        Save(dest, page, true);
    }

    template <typename T>
    size_t GetRecurseLevel(const T& p) {
        return count(p.begin(), p.end(), '/');
    }

    void RenderArticle(const ArticleInfo& ai) {
        RenderCtx ctx;
        ctx.current = ai.article;
        ctx.url_recuse_level = GetRecurseLevel(
            ai.article->GetMetadata()->relative_url);

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

            auto meta = ai.article->GetMetadata();
            auto template_name = meta->tmplte;
            if (template_name.empty()) {
                template_name= "article.html";
            }

            string article = LoadTemplate(template_name);
            map<string, string> vars;
            AssignDefauls(vars, ctx);
            Assign(*meta, vars, ctx);
            AssignHeaderAndFooter(vars, ctx);
            vars["content"] = content.str();
            vars["author"] = RenderAuthors(ai.article->GetAuthors(), ctx);
            vars["authors"] = vars["author"];
            ProcessTemplate(article, vars);
            Save(ai.tmp_path, article, true);
        }
    }

    void RenderSerie(const serie_t& serie) {
        RenderCtx ctx;
        ctx.current = serie;
        ctx.url_recuse_level = GetRecurseLevel(
            serie->GetMetadata()->relative_url);

        string series = LoadTemplate("series.html");

        const auto meta = serie->GetMetadata();
        path dst = tmp_path_;
        dst /= meta->relative_url;

        LOG_TRACE << "Generating " << *serie << " --> " << dst;


        std::map<std::string, std::string> vars;
        vars["article-type"] = boost::lexical_cast<string>(serie->GetType());
        AssignDefauls(vars, ctx);
        AssignHeaderAndFooter(vars, ctx);
        Assign(*meta, vars, ctx);

        auto articles = serie->GetArticles();
        vars["list-articles"] = RenderNodeList(articles, ctx);

        ProcessTemplate(series, vars);
        Save(dst, series, true);
    }

    void AssignDefauls(map<string, string>& vars, const RenderCtx& ctx,
                       bool skipMenu = false) {
        vars["now"] = ToStringLocal(now_);
        vars["now-ansi"] = ToStringAnsi(now_);
        vars["site-title"] = options_.options.get<string>("name", "Anonymous Nest");
        vars["site-abstract"] = options_.options.get<string>("abstract");
        vars["site-url"] = options_.options.get<string>(
            "url", options_.destination_path + "index.html");
        vars["program-name"] = PROGRAM_NAME;
        vars["program-version"] = PROGRAM_VERSION;
        vars["rel"] = ctx.GetRelativeUrl(""s);
        vars["lang"] = options_.options.get<string>("language", "en");

        if (!skipMenu) {
            vars["menu"] = RenderMenu(ctx);
        }
    }

    void Assign(const Node::Metadata& md, map<string, string>& vars, const RenderCtx& ctx) {
        vars["updated"] = ToStringLocal(md.updated);
        vars["published"] = ToStringLocal(md.published);
        vars["expires"] = ToStringLocal(md.expires);
        vars["updated-ansi"] = ToStringAnsi(md.updated);
        vars["published-ansi"] = ToStringAnsi(md.published);
        vars["expires-ansi"] = ToStringAnsi(md.expires);
        vars["title"] = stbl::ToString(md.title);
        vars["abstract"] = md.abstract;
        vars["url"] = ctx.GetRelativeUrl(md.relative_url);
        vars["tags"] = RenderTagList(md.tags, ctx);
    }

    void CommitToDestination()
    {
        // TODO: Copy only files that have changed.
        // Make checksums for all the files in the tmp site.
        // Make checksums of the files in the destination site.

        CopyDirectory(tmp_path_, options_.destination_path);
    }

    void Publish() {
        string cmd = options_.options.get<string>("publish.command");

        map<string, string> vars;
        vars["tmp-site"] = tmp_path_.string();
        vars["local-site"] = options_.destination_path;
        vars["destination"] = options_.publish_destination;

        ProcessTemplate(cmd, vars);
        LOG_INFO << "Executing shell command: \"" << cmd << "\"";
        system(cmd.c_str());
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

        set<wstring> tags;

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

        // Sort, oldest first
        sort(publishable.begin(), publishable.end(),
             [](const auto& left, const auto& right) {
                 return left->GetMetadata()->updated < right->GetMetadata()->updated;
             });


        for(const auto& a : publishable) {
            DoAddArticle(a, series);

            // Collect tags from the article
            for(const auto& tag: a->GetMetadata()->tags) {
                tags.insert(ToKey(tag));
            }
        }

        auto meta = node->GetMetadata();
        meta->relative_url = meta->article_path_part + "/index.html";

        articles_for_frontpages_.push_back(node);
        all_series_.push_back(node);

        // Add all tags from all our published articles to the series
        for(const auto tag : tags) {
            meta->tags.push_back(tag);
        }
        AddTags(meta->tags, node);

        return true;
    }

    bool AddArticle(const article_t& article) {
        if (!Validate(article)) {
            return false;
        }

        DoAddArticle(article);

        auto meta = article->GetMetadata();
        if (meta->type != "info") {
            articles_for_frontpages_.push_back(article);
        }

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

        if (meta->type != "info") {
            AddTags(meta->tags, article);
        } else {
            if (!meta->tags.empty()) {
                LOG_WARN << "The article " << ai->relative_url
                    << " has tags, but it is of type INFO - so all tags will be ignored!";
            }
        }

        if (!meta->menu.empty()) {
            AddToMenu(meta->menu, meta->relative_url);
        }
    }

    void AddTags(const vector<wstring>& tags, const node_t& node) {
        for(const auto& tag : tags) {
            auto key = ToKey(tag);

            // Preserve caps from the first time we encounter a tag
            if (tags_.find(key) == tags_.end()) {
                tags_[key].name = stbl::ToString(tag);
            }

            tags_[key].nodes.push_back(node);
        }
    }

    wstring ToKey(wstring name) {
        transform(name.begin(), name.end(), name.begin(), ::tolower);
        return name;
    }

    void RenderFrontpage() {
        RenderCtx ctx;
        string frontpage = LoadTemplate("frontpage.html");

        std::map<std::string, std::string> vars;

        AssignDefauls(vars, ctx);

        vars["now-ansi"] = ToStringAnsi(now_);
        vars["title"] = vars["site-title"];
        vars["abstract"] = vars["site-abstract"];
        vars["url"] = vars["site-url"];

        AssignHeaderAndFooter(vars, ctx);

        auto articles = articles_for_frontpages_;
        sort(articles.begin(), articles.end(),
             [](const auto& left, const auto& right) {
                 return left->GetMetadata()->updated > right->GetMetadata()->updated;
             });

        vars["list-articles"] = RenderNodeList(articles, ctx);

        {
            vector<wstring> tags;
            for(const auto& t: tags_) {
                tags.push_back(t.first);
            }

            vars["tags"] = RenderTagList(tags, ctx);
        }

        ProcessTemplate(frontpage, vars);

        path frontpage_path = tmp_path_;
        frontpage_path /= "index.html";
        Save(frontpage_path, frontpage);
    }

    void AssignHeaderAndFooter(std::map<std::string, std::string>& vars,
                               const RenderCtx& ctx) {
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
                          const RenderCtx& ctx) {
        std::stringstream out;

        for(const auto& n : nodes) {
            map<string, string> vars;
            AssignDefauls(vars, ctx);
            const auto meta = n->GetMetadata();
            vars["article-type"] = boost::lexical_cast<string>(n->GetType());
            Assign(*meta, vars, ctx);
            string item = LoadTemplate("article-in-list.html");
            ProcessTemplate(item, vars);
            out << item << endl;
        }

        return out.str();
    }

    template <typename TagList>
    string RenderTagList(const TagList& tags, const RenderCtx& ctx) {

        std::stringstream out;

        for(const auto& tag : tags) {
            map<string, string> vars;
            AssignDefauls(vars, ctx);

            auto key = ToKey(tag);
            auto tag_info = tags_[key];

            vars["url"] = ctx.GetRelativeUrl(tag_info.url);
            vars["name"] = stbl::ToString(tag);

            string tmplte = LoadTemplate("tag.html");
            ProcessTemplate(tmplte, vars);
            out << tmplte << endl;
        }

        return out.str();
    }

    string RenderMenu(const RenderCtx& ctx) {
        map<string, string> vars;
        AssignDefauls(vars, ctx, true);
        string tmplte = LoadTemplate("menu.html");
        vars["content"] = RenderMenu(menu_.children, ctx);
        ProcessTemplate(tmplte, vars);
        return tmplte;
    }

    string RenderMenu(const vector<shared_ptr<Menu>>& menus, const RenderCtx& ctx) {
        std::stringstream out;
        for(const auto& menu : menus) {
            map<string, string> vars;
            AssignDefauls(vars, ctx, true);
            string tmplte;

            if (!menu->url.empty()) {
                tmplte = LoadTemplate("menuitem.html");
                // TODO: expand macros (like {{rel} and {{site-url}})
                // TODO: Check if it's an absolute url
                vars["url"] = ctx.GetRelativeUrl(menu->url);
            } else if (!menu->children.empty()){
                tmplte = LoadTemplate("submenu.html");
                vars["content"] = RenderMenu(menu->children, ctx);
            } else {
                LOG_WARN << "Menu ... " << stbl::ToString(menu->name)
                    << "Has neither a URL nor sub-menus!";
                return {};
            }

            vars["name"] = stbl::ToString(menu->name);
            ProcessTemplate(tmplte, vars);
            out << tmplte << endl;
        }

        return out.str();
    }

    string RenderAuthors(const Article::authors_t& authors, const RenderCtx& ctx) {

        std::stringstream out;

        for(const auto& key : authors) {
            string full_key = "people."s + key;
            map<string, string> vars;
            AssignDefauls(vars, ctx);

            if (options_.options.get_child_optional(full_key)) {

                vars["name"] = options_.options.get<string>(full_key + ".name", key);
                string email = options_.options.get<string>(full_key + ".email", "");
                if (!email.empty()) {
                    vars["email"] = R"(<a class="author" href="mailto:)"s + email + R"(">)"s
                        + email + "</a>";
                }


                std::vector<string> handles;
                for(const auto& it :  options_.options.get_child(full_key)) {
                    if ((it.first == "name") || (it.first == "email")) {
                        continue;
                    }

                    map<string, string> hvars;
                    AssignDefauls(hvars, ctx);
                    hvars["handle"] = it.first;
                    hvars["name"] = it.second.get("name", it.first);
                    hvars["url"] = it.second.get("url", "");
                    hvars["icon"] = it.second.get("icon", ctx.GetRelativeUrl("www.svg"));

                    auto handle_template = LoadTemplate("social-handle.html");
                    handles.push_back(ProcessTemplate(handle_template, hvars));
                }

                if (!handles.empty()) {
                    std::stringstream hout;

                    for(const auto& h : handles) {
                        hout << h;
                    }

                    map<string, string> hvars;
                    AssignDefauls(hvars, ctx);

                    hvars["handles"] = hout.str();
                    auto handles_template = LoadTemplate("social_handles.html");
                    vars["social-handles"] = ProcessTemplate(handles_template, hvars);
                }
            } else {
                vars["name"] = key;
            }

            string tmplte = LoadTemplate("author.html");
            ProcessTemplate(tmplte, vars);
            out << tmplte << endl;
        }

        return out.str();
    }

    string ToStringAnsi(const time_t& when) {
        std::tm tm = *std::localtime(&when);
        return boost::lexical_cast<string>(put_time(&tm, "%F %R"));
    }

    string ToStringLocal(const time_t& when) {
        static const string format = options_.options.get<string>("system.date.format", "%c");
        std::tm tm = *std::localtime(&when);
        return boost::lexical_cast<string>(put_time(&tm, format.c_str()));
    }

    string& ProcessTemplate(string& tmplte,
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
        return tmplte;
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

    // All tags from all content
    map<std::wstring, TagInfo> tags_;

    // Root menu item
    Menu menu_;

    path tmp_path_;

    const time_t now_;
};

std::shared_ptr<ContentManager> ContentManager::Create(const Options& options)
{
    return make_shared<ContentManagerImpl>(options);
}

}

