
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
#include <future>
#include <thread>

#include <boost/asio.hpp>
#include <boost/lexical_cast.hpp>
#include <boost/algorithm/string.hpp>
#include <boost/lexical_cast.hpp>
#include <boost/algorithm/string/split.hpp>
#include <boost/algorithm/string.hpp>
#include <boost/regex.hpp>
#include <boost/asio/thread_pool.hpp>

#include "stbl/Options.h"
#include "stbl/ContentManager.h"
#include "stbl/Scanner.h"
#include "stbl/Node.h"
#include "stbl/Series.h"
#include "stbl/ImageMgr.h"
#include "stbl/Sitemap.h"
#include "stbl/logging.h"
#include "stbl/utility.h"
#include "templates_res.h"
#include "stbl/stbl_config.h"
#include "stbl/utility.h"
#include "stbl/pipe.h"

using namespace std;
using namespace std::filesystem;
using namespace std::string_literals;
namespace ba = boost::asio;

namespace stbl {

ContentManager *ContentManager::self_ = nullptr;

class ContentManagerImpl final : public ContentManager
{
public:
    struct ArticleInfo {
        article_t article;
        string relative_url; // Relative to the websites root
        path tmp_path;
        path dst_path;
    };

    struct TagInfo {

        void sort() {
            std::sort(nodes.begin(), nodes.end(), [](const auto& left, const auto& right) {
                return left->GetMetadata()->latestDate() > right->GetMetadata()->latestDate();
            });
        }

        nodes_t nodes;
        string name; //utf8 with caps as in first seen version
        string url;
    };



    struct Menu {
        wstring name;
        string url;
        vector<shared_ptr<Menu>> children;
    };

    ContentManagerImpl(const Options& options)
    : now_{time(nullptr)}
    , roundup_{options.options.get<time_t>("system.date.roundup", 1800)}
    , num_threads_{options.threads == -1
            ? std::max<size_t>(std::thread::hardware_concurrency() - 1, 2)
            : static_cast<size_t>(std::max<int>(options.threads, 1))}
    {
        options_ = options;
        assert(!self_);
        self_ = this;
        if (auto chroma = options.options.get_optional<string>("chroma.enabled")) {
            auto command = options.options.get_optional<string>("chroma.path");
            if (!command) {
                command = "chroma";
            };
            if (*chroma == "auto") {
                auto query = *command + " -h";
                if (std::system(query.c_str()) == 0) {
                    chroma = "true";
                } else {
                    LOG_INFO << "Chroma not found. I will not syntax highlight source code.";
                }
            }

            if (*chroma == "true") {
                syntax_highlighter_ = *command;
            } else {
                LOG_WARN << "No syntax highlighter specified.";
            }
        }

        LOG_INFO << "Running with " << num_threads_ << " threads";
    }

    ~ContentManagerImpl() {
        CleanUp();
        self_ = nullptr;
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
        LOG_DEBUG << "Waiting for worker-threads to finish";
        pool_.join();
    }


protected:
    void Scan()
    {
        scanner_ = Scanner::Create(options_);

        {
            string str_widths = options_.options.get<string>("banner.widths",
                                              "94, 248, 480, 640, 720, 950");
            vector<string> values;
            boost::split(values, str_widths, boost::is_any_of(" ,"));
            ImageMgr::widths_t widths;
            for(const auto& v: values) {
                if (v.empty()) {
                    continue;
                }
                widths.push_back(stoi(v));
            }

            images_ = ImageMgr::Create(widths,
                options_.options.get<int>("banner.quality", 95));
        }
        nodes_ = scanner_->Scan();

        LOG_DEBUG << "Listing nodes after scan: ";
        for(const auto& n: Nodes()) {
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

        tmp_path__ = MkTmpPath();
        create_directories(GetTmpPath());

        for(const auto& n: Nodes()) {
            switch(n->GetType()) {
                case Node::Type::SERIES: {
                    auto s = dynamic_pointer_cast<Series>(n);
                    assert(s);
                    AddSeries(s);
                } break;
                case Node::Type::ARTICLE: {
                    auto a = dynamic_pointer_cast<Article>(n);
                    assert(a);
                    if (a->GetMetadata()->type == "index"s) {
                        index_ = a;
                    } else {
                        AddArticle(a);
                    }
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
                    LOG_WARN << "Overriding existing menu \"" << toString(name)
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

                current_menu->children.push_back(std::move(new_menu));
                current_menu = current_menu->children.back().get();

            } while(depth < parts.size());

            return; // never continue the outer loop at this point
        }
    }

    void MakeTempSite()
    {
        std::vector<string> directories_to_copy{
            "images", "video", "artifacts", "files"
        };

        sitemap_ = Sitemap::Create();

        // Render the articles in parallel
        vector<std::pair<ba::awaitable<void>, string>> tasks;

        tasks.emplace_back(RenderFrontpage(), "RenderFrontpage");

        // Queue tasks
        for (const auto& ai : all_articles_) {
            auto name = format("Article: {}", toString(ai->article->GetMetadata()->title));
            tasks.emplace_back(RenderArticle(*ai), name);
        }

        // Queue series tasks
        for (const auto& n : all_series_) {
            tasks.emplace_back(RenderSerie(n), "RenderSerie");
        }

        // Queue tag tasks
        for (const auto& t : tags_) {
            tasks.emplace_back(RenderTag(t.second), "RenderTag");
        }

        // We need to spawn each task separately
        //std::vector<boost::asio::awaitable<void>> spawned_tasks;
        //spawned_tasks.reserve(tasks.size());
        vector<std::future<void>> futures;
        futures.reserve(tasks.size());

        for (auto& task : tasks) {
            futures.emplace_back(co_spawn(pool_, std::move(task.first), ba::use_future));
        }

        LOG_DEBUG << "Waiting for worker-threads to finish";
        size_t remaining_tasks = futures.size();
        while(remaining_tasks) {
            LOG_TRACE << "Looping for worker-threads to finish. "
                      << "There are " << remaining_tasks << " tasks left.";
            auto index = 0;
            for(auto& f : futures) {
                if (f.valid()) {
                    try {
                        LOG_TRACE << "Waiting for worker-thread to finish task: "
                                  << tasks.at(index).second
                                  << " There are " << remaining_tasks << " tasks left.";
                        if (f.wait_for(200ms) == future_status::ready) {
                            f.get();
                            --remaining_tasks;
                        }
                    } catch (const std::exception& e) {
                        LOG_ERROR << "Error in worker-thread: " << e.what();
                        --remaining_tasks;
                    }
                }
                ++index;
            }
        }

        LOG_DEBUG << "All worker-threads finished. Shutting down thread pool";

        //pool.stop();
        //pool.join();

        LOG_DEBUG << "Thread pool shut down";

        // Create sitemap
        {
            auto sitemap = GetTmpPath();
            sitemap /= "sitemap.xml";
            sitemap_->Write(sitemap);
        }

        // Copy artifacts, images and other files
        for(const auto& d : directories_to_copy) {
            path src = options_.source_path, dst = GetTmpPath();
            src /= d;
            dst /= d;
            if (std::filesystem::is_directory(src)) {
                CopyDirectory(src, dst);
            } else {
                LOG_WARN << "Cannot copy directory " << src
                    << ", it does not exist.";
            }
        }

        // Handle special files
        {
            auto dst = GetTmpPath();
            auto favicon = dst;
            favicon /= "artifacts";
            favicon /= "favicon.ico";
            if (std::filesystem::is_regular_file(favicon)) {
                dst /= "favicon.ico";

                if (std::filesystem::is_regular_file(dst)) {
                    LOG_TRACE << "Removing existing file: " << dst;
                    std::filesystem::remove(dst);
                }
                LOG_TRACE << "Copying " << favicon << " --> " << dst;
                std::filesystem::copy(favicon, dst);
            }
        }

        auto robots = GetTmpPath();
        robots /= "robots.txt";
        if (!std::filesystem::is_regular_file(robots)) {
            std::stringstream out;
            out << "Sitemap: " << GetSiteUrl() << "/sitemap.xml" << endl
                << "User-agent: *" << endl
                << "Disallow: /files" << endl;
            Save(robots, out.str());
        }
    }

    void RenderRss(const nodes_t& articles,
                   std::filesystem::path path,
                   const std::string& title,
                   const std::string& description,
                   const std::string& link,
                   const std::string& rss_link) {

        if (!options_.options.get<bool>("rss.enabled", true)) {
            LOG_TRACE << "RSS is disabled. Not generating RSS for: " << link;
            return;
        }

        std::stringstream out;
        out << R"(<?xml version="1.0" encoding="UTF-8" ?>)" << endl
            << R"(<rss version="2.0" xmlns:atom="http://www.w3.org/2005/Atom">)" << endl
            << "<channel>" << endl
            << R"(<atom:link href=")"
                << rss_link
                << R"(" rel="self" type="application/rss+xml" />)" << endl
            << "<title>" << escapeForXml(title) << "</title>" << endl
            << "<description>" << escapeForXml(description) << "</description>" << endl
            << "<link>" << link << "</link>" << endl
            << "<lastBuildDate>" << RssTime(time(nullptr)) << "</lastBuildDate>" << endl
            << "<pubDate>" << RssTime(time(nullptr)) << "</pubDate>" << endl
            << "<ttl>" << options_.options.get<unsigned>("rss.ttl", 1800) << "</ttl>" << endl;

        for (const auto &a : articles) {
            auto hdr = a->GetMetadata();

            const auto url = GetSiteUrl() + "/"s + hdr->relative_url;

            out << "<item>" << endl
                << " <title>" << escapeForXml(ToString(hdr->title)) << "</title>" << endl
                << " <description>" << escapeForXml(hdr->abstract) << "</description>" << endl
                << " <link>" << url << "</link>" << endl
                << R"( <guid isPermaLink="false">)" << hdr->uuid << "</guid>" << endl
                << " <pubDate>" << RssTime(hdr->published) << "</pubDate>" << endl
                << "</item>" << endl;
        }

        out << "</channel>" << endl
            << "</rss>" << endl;

        // Use the same file-name as the link, but with another extention
        path.replace_extension("rss");
        LOG_DEBUG << "Creating RSS feed " << path;
        Save(path, out.str());
    }

    // Return a date like: Sat, 07 Sep 2002 0:00:01 GMT
    string RssTime(const time_t when) {
        if (!when) {
            return {};
        }

        // RFC 822 was written before languages other than US English was invented...

        static array<const char *, 7> days = {
             "Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"};

        static array <const char *, 12> months = {
             "Jan", "Feb",  "Mar", "Apr", "May", "Jun", "Jul", "Aug",
             "Sep", "Oct", "Nov", "Dec"};

        const auto *tm = gmtime(&when);
        if (tm == nullptr) {
            throw runtime_error("Invalid date after conversion by gmtime");
        }

        stringstream out;
        out << days.at(tm->tm_wday) << ", "
            << std::setfill('0') << std::setw(2) << tm->tm_mday
            << std::setw(0) << ' ' << months.at(tm->tm_mon)
            << ' ' << std::setw(4) << (tm->tm_year + 1900)
            << std::setw(0) << ' ' << std::setw(2) << tm->tm_hour
            << std::setw(0) << ':' << std::setw(2) << tm->tm_min
            << std::setw(0) << ':' << std::setw(2) << tm->tm_sec
            << " GMT";

        return out.str();
    }

    ba::awaitable<void> RenderTag(const TagInfo& ti) {
        if (ti.nodes.empty()) {
            // Not used
            LOG_TRACE << "Ignoring unused tag.";
            co_return;
        }

        RenderCtx ctx;
        ctx.url_recuse_level = GetRecurseLevel(ti.url);

        auto page = LoadTemplate("tags.html");

        map<string, string> vars;
        AssignDefauls(vars, ctx);
        vars["name"] = ti.name;
        vars["title"] = ti.name;
        vars["url"] = ctx.GetRelativeUrl(ti.url);
        vars["page-url"] = GetSiteUrl() + "/" + ti.url;
        AssignHeaderAndFooter(vars, ctx);
        vars["list-articles"] = RenderNodeList(ti.nodes, ctx);
        ProcessTemplate(page, vars);

        path dest = GetTmpPath();
        dest /= ti.url;
        Save(dest, page, true);

        Sitemap::Entry sm_entry;
        sm_entry.priority = GetSitemapPriority("tag");
        sm_entry.url = vars["page-url"];
        sm_entry.updated = ToStringAnsi(Roundup(now_, roundup_));
        sitemap_->Add(sm_entry);
        co_return;
    }

    template <typename T>
    size_t GetRecurseLevel(const T& p) {
        return count(p.begin(), p.end(), '/');
    }

    void handleVideo(const Page& page,
                     map<string, string>& vars) {
        if (page.containsVideo()) {
            const auto css = options_.options.get("plyr.css", ""s);
            const auto js = options_.options.get("plyr.js", ""s);
            if (!css.empty() && !js.empty()) {
                vars["plyr-css"] = format(R"(<link rel="stylesheet" href="{}"/>)", css);
                vars["plyr-js-load"] = format(R"(<script src="{}"></script>)", js);
                vars["plyr-js-run"] = format(R"(<script>
  document.addEventListener('DOMContentLoaded', () => {{
    const configs = {{}};
    configs.forEach(cfg => {{
      new Plyr(cfg.selector, cfg.options);
      if (cfg.portrait) {{
        player.elements.container.classList.add('plyr--portrait');
      }}
    }});
  }});
</script>)", page.getVideOptions());
            }
        }
    }


    boost::asio::awaitable<void> RenderArticle(const ArticleInfo& ai) {
        RenderCtx ctx;
        ctx.current = ai.article;
        ctx.url_recuse_level = GetRecurseLevel(ai.article->GetMetadata()->relative_url);

        auto meta = ai.article->GetMetadata();

        if (meta->uuid == "7c7aca65-7a7a-416a-b9e3-b9e090f0b9c7") {
            int i = 1;
        }

        // TODO: Handle multiple pages
        for(auto& p : ai.article->GetContent()->GetPages()) {

            LOG_DEBUG << "Generating " << *ai.article
                << " --> " << ai.tmp_path;

            const auto directory = ai.tmp_path.parent_path();
            if (!is_directory(directory)) {
                create_directories(directory);
            }

            stringstream content;
            const auto words = co_await p->Render2Html(content, ctx);
            string content_str = content.str();

            LOG_INFO << "Article " << *ai.article << " contains " << words << " words.";

            auto template_name = meta->tmplte;
            if (template_name.empty()) {
                template_name= "article.html";
            }

            // syntax highlighting
            if (content_str.find("<code class=") != string::npos) {
                co_await SyntaxHighlight(content_str);
            }

            string article = LoadTemplate(template_name);
            map<string, string> vars;
            vars["minutes-to-read"] = to_string(max<int>(1, words / 275));
            AssignDefauls(vars, ctx);
            handleVideo(*p, vars);
            Assign(*meta, vars, ctx);
            AssignHeaderAndFooter(vars, ctx);
            AssignNavigation(vars, *ai.article, ctx);
            vars["content"] = std::move(content_str);
            auto authors = ai.article->GetAuthors();
            if (authors.empty()) {
                auto default_author = options_.options.get<string>("people.default", "");
                if (!default_author.empty()) {
                    authors.push_back(std::move(default_author));
                }
            }
            vars["author"] = RenderAuthors(authors, ctx);
            vars["authors"] = vars["author"];
            if (!meta->banner.empty()) {
                vars["banner"] = RenderBanner(*meta, ctx);
            }

            vars["read-time"] = Render("read-time.html", vars, ctx);

            if (article.find("{{list-articles}}") != string::npos) {
                RenderArticleIntros(ctx, vars, ai.tmp_path);
            } else {
                ProcessTemplate(article, vars);
                Save(ai.tmp_path, article, true);
            }

            Sitemap::Entry sm_entry;
            sm_entry.priority = GetSitemapPriority("article",
                static_cast<float>(meta->sitemap_priority) / 100.0);
            sm_entry.changefreq = meta->sitemap_changefreq;
            sm_entry.url = vars["page-url"];
            sm_entry.updated = vars["updated-ansi"];
            sitemap_->Add(sm_entry);
        }

        if (options_.update_source_headers) {
            if (ai.article->GetMetadata()->type != "index"s) {
                ai.article->UpdateSourceHeaders(*scanner_, *meta);
            }
        }

        co_return;
    }

    void AssignNavigation(map<string, string>& vars, const Article& article,
                          const RenderCtx& ctx) {

        if (auto series = article.GetSeries()) {
            auto articles = series->GetArticles();
            Wash(articles);

            node_t next, prev;
            auto uuid = article.GetMetadata()->uuid;
            for(auto it = articles.begin(); it != articles.end(); ++it) {
                if ((*it)->GetMetadata()->uuid == uuid) {
                    if (it != articles.begin()) {
                        prev = *(it - 1);
                    }
                    auto nit = it + 1;
                    if (nit != articles.end()) {
                        next = *nit;
                    }
                    break;
                }
            }

            if (prev) {
                vars["prev"] = prev->GetMetadata()->relative_url;
                vars["if-prev"] = Render("prev.html", vars, ctx);
            }

            if (next) {
                vars["next"] = next->GetMetadata()->relative_url;
                vars["if-next"] = Render("next.html", vars, ctx);
            }

            vars["up"] = series->GetMetadata()->relative_url;
            vars["if-up"] = Render("up.html", vars, ctx);
        }
    }

    void Wash(articles_t& articles) {
        articles.erase(remove_if(articles.begin(), articles.end(), [this](const article_t& a) {
            const auto meta = a->GetMetadata();
            return !(options_.preview_mode ? true : meta->is_published)
                || (meta->type == "index"s);
        }));
    }

    string RenderBanner(const Node::Metadata& meta, const RenderCtx& ctx) {
        static const int align = options_.options.get<int>("banner.align", 0);

        path image_path = options_.source_path;
        image_path /= "images";
        image_path /= meta.banner;

        auto imgs = images_->Prepare(image_path);

        stringstream out;
        string default_src;

        out << R"(<picture class="banner">)" << endl;
        for (const auto &v : imgs) {
            if (default_src.empty() && (v.size.width >= 300)) {
                default_src = v.relative_path;
                break;
            }
        }

        for(auto it = imgs.begin(); it != imgs.end(); ++it) {
            const int width = it->size.width + align;
            out << "<source media=\"(min-width: "
                <<  width << "px)\" srcset=\""
                << ctx.GetRelativeUrl(it->relative_path)
                << "\">" << endl;
        }

        if (!default_src.empty()) {
            out << R"(<img src=")" << ctx.GetRelativeUrl(default_src) << R"(" alt="Banner">)" << endl;
        }
        out << "</picture>" << endl;
        return out.str();
    }

    ba::awaitable<void> RenderSerie(const serie_t& serie) {
        RenderCtx ctx;
        ctx.current = serie;
        ctx.url_recuse_level = GetRecurseLevel(
            serie->GetMetadata()->relative_url);

        string series = LoadTemplate("series.html");

        const auto meta = serie->GetMetadata();
        path dst = GetTmpPath();
        dst /= meta->relative_url;

        LOG_TRACE << "Generating " << *serie << " --> " << dst;


        std::map<std::string, std::string> vars;
        vars["article-type"] = boost::lexical_cast<string>(serie->GetType());
        AssignDefauls(vars, ctx);

        Sitemap::Entry sm_entry;
        sm_entry.priority = GetSitemapPriority("series");
        sm_entry.url = vars["page-url"];
        sm_entry.updated = vars["updated-ansi"];

        auto articles = serie->GetArticles();
        for(const auto& a: articles) {
            const auto am = a->GetMetadata();
            if (am->type == "index"s) {
                if (auto content = a->GetContent()) {
                    auto pages = content->GetPages();
                    if (!pages.empty()) {
                        LOG_TRACE << "Adding content to cover-page";
                        auto p = pages.front();
                        stringstream content;
                        co_await p->Render2Html(content, ctx);
                        handleVideo(*p, vars);
                        vars["content"] = content.str();
                    }

                    if (!am->title.empty()) {
                        meta->title = am->title;
                    }
                    meta->abstract = am->abstract;
                    meta->banner = am->banner;
                    if (!meta->banner.empty()) {
                        vars["banner"] = RenderBanner(*meta, ctx);
                    }

                    if (meta->sitemap_priority >= 0) {
                        sm_entry.priority = static_cast<float>(
                            meta->sitemap_priority) / 100.0;
                    }

                    sm_entry.changefreq = meta->sitemap_changefreq;
                }
                break;
            }
            co_return;
        }

        Assign(*meta, vars, ctx);
        AssignHeaderAndFooter(vars, ctx);
        Wash(articles);
        vars["list-articles"] = RenderNodeList(articles, ctx);

        ProcessTemplate(series, vars);
        Save(dst, series, true);
        sitemap_->Add(sm_entry);
    }

    void AssignDefauls(map<string, string>& vars, const RenderCtx& ctx,
                       bool skipMenu = false) {
        vars["now"] = ToStringLocal(now_);
        vars["now-year"] = GetCurrentYear();
        vars["now-ansi"] = ToStringAnsi(now_);
        vars["site-title"] = options_.options.get<string>("name", "Anonymous Nest");
        vars["site-abstract"] = options_.options.get<string>("abstract");
        vars["site-url"] = GetSiteUrl();
        vars["program-name"] = PROGRAM_NAME;
        vars["program-version"] = STBL_VERSION;
        vars["rel"] = ctx.GetRelativeUrl(""s);
        vars["lang"] = options_.options.get<string>("language", "en");
        vars["scripts"] = RenderScripts(ctx);
        vars["rss"] = "index.rss";

        if (!skipMenu) {
            vars["menu"] = RenderMenu(ctx);
        }
    }

    string GetSiteUrl() const {
        static const string site_url = ComputeSiteUrl();
        return site_url;
    }

    string ComputeSiteUrl() const {
        string url = options_.options.get<string>(
            "url", options_.destination_path);
        if (!url.empty() && url[url.size() -1] == '/') {
            url.resize(url.size() -1);
        }
        return url;
    }

    // Load scripts in 'scrips' folder in ascending order
    string RenderScripts(const RenderCtx& ctx) {
        static const string scripts = GetScripts(ctx);
        return scripts;
    }

    string GetScripts(const RenderCtx& ctx) {
        stringstream out;

        std::vector<path> paths;

        path scripts = options_.source_path;
        scripts /= "scripts";
        if (std::filesystem::is_directory(scripts)) {
            for (const auto& de : std::filesystem::directory_iterator{scripts}) {
                paths.push_back(de.path());
            }

            sort(paths.begin(), paths.end());

            for(const auto& path : paths) {
                out << Load(path);
            }
        }

        return out.str();
    }

    void Assign(const Node::Metadata& md, map<string, string>& vars, const RenderCtx& ctx) {

        vars["updated"] = ToStringLocal(Roundup(md.updated, roundup_));
        vars["published"] = ToStringLocal(Roundup(md.published, roundup_));
        vars["expires"] = ToStringLocal(md.expires);
        vars["updated-ansi"] = ToStringAnsi(Roundup(md.updated, roundup_));
        vars["published-ansi"] = ToStringAnsi(Roundup(md.published, roundup_));
        vars["expires-ansi"] = ToStringAnsi(md.expires);
        vars["title"] = stbl::ToString(md.title);
        vars["abstract"] = md.abstract;
        vars["url"] = ctx.GetRelativeUrl(md.relative_url);
        vars["page-url"] = GetSiteUrl() + "/" + md.relative_url;
        vars["tags"] = RenderTagList(md.tags, ctx);
        vars["uuid"] = md.uuid;
        vars["comments"] = RenderComments(md, vars, ctx);
        vars["banner-credits"] = md.banner_credits;
        vars["pubdate"] = Render("pubdate.html", vars, ctx);
        if (const auto& when = vars["updated"]; !when.empty() && when != vars["published"]) {
            vars["updatedate"] = Render("updatedate.html", vars, ctx);
            vars["if-updated"] = vars["updatedate"];
            vars["pubished-or-updated"] = vars["updatedate"];
        } else {
            vars["pubished-or-updated"] = vars["pubdate"];
        }
        vars["pubdates"] = Render("pubdates.html", vars, ctx);
        vars["og-image"] = RenderOgImage(md, vars, ctx);

        if (!md.abstract.empty()) {
            vars["og-description"] = R"(<meta property="og:description" content=")"s + md.abstract + R"("/>)";
        }
    }

    string RenderOgImage(const Node::Metadata& md,
                         map<string, string>& vars,
                         const RenderCtx& ctx) {
        if (md.banner.empty()) {
            return {};
        }

        auto path = GetSiteUrl() + "/images/" + md.banner;
        return R"(<meta property="og:image" content=")"s + path + R"("/>)";
    }

    string RenderComments(const Node::Metadata& md, map<string, string>& vars, const RenderCtx& ctx) {
        if (md.comments == "no") {
            return {};
        }

        auto comments = md.comments;
        if (comments.empty()) {
            comments = options_.options.get("comments.default", "");
        }

        if (comments.empty()) {
            return {};
        }

        const auto key = "comments."s + comments;


        if (auto node = options_.options.get_child_optional(key)) {
            for(const auto it :  *node) {
                vars[comments + "-" + it.first] =  it.second.get("", "");
            }
        } else {
            // No comments
            return {};
        }

        string tmplte_file = options_.options.get(key + ".template", "");
        if (tmplte_file.empty()) {
            return {};
        }

        return Render(tmplte_file, vars, ctx);
    }

    string Render(const string& templateName,
                  map<string, string>& vars,
                  const RenderCtx& ctx) {
        auto tmplte = LoadTemplate(templateName);

        ProcessTemplate(tmplte, vars);
        return tmplte;
    }

    void CommitToDestination()
    {
        if (std::filesystem::is_directory(options_.destination_path)) {
            LOG_DEBUG << "Deleting directory: " << options_.destination_path;
            std::filesystem::remove_all(options_.destination_path);
        }

        CopyDirectory(GetTmpPath(), options_.destination_path);
    }

    void Publish() {
        string cmd = options_.options.get<string>("publish.command");

        map<string, string> vars;
        vars["tmp-site"] = GetTmpPath().string();
        vars["local-site"] = options_.destination_path;
        vars["destination"] = options_.publish_destination;

        ProcessTemplate(cmd, vars);
        LOG_INFO << "Executing shell command: \"" << cmd << "\"";
        (void) system(cmd.c_str());
    }

    void CleanUp()
    {
        // Remove the temp site
        if (!options_.keep_tmp_dir && !GetTmpPath().empty() && is_directory(GetTmpPath())) {
            LOG_DEBUG << "Removing temporary directory " << GetTmpPath();
            remove_all(GetTmpPath());
        }

        // Remove any other temporary files
    }

    bool Validate(const node_t& node) {
        const auto meta = node->GetMetadata();
        const auto now = time(NULL);

        LOG_TRACE << "Evaluating " << *node << " for publishing...";

        if (!meta->is_published) {
            LOG_INFO << *node << " is held back because it is unpublished.";
            return options_.preview_mode ? true : false;
        }

        if (meta->published > now) {
            LOG_INFO << *node
                << " is held back because it is due to be published at "
                << put_time(localtime(&meta->published), "%Y-%m-%d %H:%M");
            return options_.preview_mode ? true : false;
        }

        if (meta->expires && (meta->expires < now)) {
            LOG_INFO << *node
                << " is held back because it expired at "
                << put_time(localtime(&meta->expires), "%Y-%m-%d %H:%M");
            return options_.preview_mode ? true : false;
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
                const auto leftPart = left->GetMetadata()->part;
                const auto rightPart = right->GetMetadata()->part;
                if (leftPart && rightPart) {
                    return leftPart < rightPart;
                }

                return left->GetMetadata()->latestDate() < right->GetMetadata()->latestDate();
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
        for (const auto &tag : tags) {
            meta->tags.push_back(tag);
        }
        AddTags(meta->tags, node);

        meta->updated = publishable.back()->GetMetadata()->updated;

        series->SetArticles(std::move(publishable));

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
        ai->tmp_path = GetTmpPath();

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

    static wstring ToKey(wstring name) {
        transform(name.begin(), name.end(), name.begin(), ::tolower);
        return name;
    }

    void RenderArticleIntros(RenderCtx ctx, std::map<std::string, std::string> vars, filesystem::path destName = {}) {
        auto fp_articles = articles_for_frontpages_;

        // don't list the article that renders the list.
        if (ctx.current) {
            const auto uuid = ctx.current->GetMetadata()->uuid;
            std::erase_if(fp_articles, [uuid = ctx.current->GetMetadata()->uuid](const auto& a) {
                return a->GetMetadata()->uuid == uuid;
            });
        }

        sort(fp_articles.begin(), fp_articles.end(),
             [](const auto& left, const auto& right) {
                 auto res = left->GetMetadata()->latestDate() - right->GetMetadata()->latestDate();
                 if (res) {
                     return res > 0;
                 }
                 return left->GetMetadata()->title > right->GetMetadata()->title;
             });

        const int max_articles = options_.options.get("max-articles-on-frontpage", 16);
        nodes_t articles;
        int page_count = 0;

        for(auto i = fp_articles.begin();; ++i) {

            if (i != fp_articles.end()) {
                articles.push_back(*i);
            }

            if ((i == fp_articles.end()) || (articles.size() >= max_articles)) {
                vars["list-articles"] = RenderNodeList(articles, ctx);

                {
                    vector<wstring> tags;
                    for(const auto& t: tags_) {
                        tags.push_back(t.first);
                    }

                    vars["tags"] = RenderTagList(tags, ctx);
                }

                if (page_count) {
                    vars["prev"] = GetArticlesPresentPageName(page_count -1, destName);
                    vars["if-prev"] = Render("prev.html", vars, ctx);
                } else {
                    vars.erase("prev");
                    vars.erase("if-prev");
                }

                if (i != fp_articles.end()) {
                    vars["next"] = GetArticlesPresentPageName(page_count +1, destName);
                    vars["if-next"] = Render("next.html", vars, ctx);
                } else {
                    vars.erase("next");
                    vars.erase("if-next");
                }

                string frontpage = LoadTemplate("frontpage.html");
                ProcessTemplate(frontpage, vars);

                const auto fp_path = GetArticlesPresentPageName(page_count, destName);
                auto dst_path = GetTmpPath().string() + "/"s + fp_path;
                LOG_DEBUG << "Generating frontpage " << dst_path;
                Save(dst_path, frontpage);
                Sitemap::Entry sm_entry;
                sm_entry.priority = GetSitemapPriority("frontpage");
                sm_entry.url = GetSiteUrl() + "/" + fp_path;
                sm_entry.updated = ToStringAnsi(Roundup(now_, roundup_));
                sitemap_->Add(sm_entry);
                ++page_count;
                articles.clear();
            }

            if (i == fp_articles.end()) {
                break;
            }
        }
    }

    ba::awaitable<void> RenderFrontpage() {
        RenderCtx ctx;
        std::map<std::string, std::string> vars;

        AssignDefauls(vars, ctx);

        vars["now-ansi"] = ToStringAnsi(now_);
        vars["title"] = vars["site-title"];
        vars["abstract"] = vars["site-abstract"];
        vars["url"] = vars["page-url"] = vars["site-url"];

        auto gsv = options_.options.get("seo.google-site-verification", "");
        if (!gsv.empty()) {
            vars["google-site-verification"] =
                R"(<meta name="google-site-verification" content=")" + gsv +
                R"("/>)";
        }

        bool generate_article_listing = true;
        if (index_) {
            auto meta = index_->GetMetadata();
            if (!meta->banner.empty()) {
                vars["banner"] = RenderBanner(*meta, ctx);
            }

            if (!meta->tmplte.empty() && (meta->tmplte != "index.html")) {
                vars["template"] = meta->tmplte;
                generate_article_listing = false;
            }

            auto pages = index_->GetContent()->GetPages();
            if (!pages.empty()) {
                LOG_TRACE << "Adding content to front-page.";
                auto p = pages.front();
                stringstream content;
                co_await p->Render2Html(content, ctx);
                handleVideo(*p, vars);
                vars["content"] = content.str();
            }

            if (!meta->abstract.empty()) {
                vars["abstract"] = meta->abstract;
            }
        }

        {
            auto base_url = vars["site-url"];
            if (!base_url.empty() && (base_url.back() == '/')) {
                base_url.resize(base_url.size() -1);
            }
            vars["rss-abs"] = base_url + "/index.rss";
        }

        AssignHeaderAndFooter(vars, ctx);

        if (generate_article_listing) {
            RenderArticleIntros(ctx, vars);
        } else {
            string article = LoadTemplate(vars["template"]);
            ProcessTemplate(article, vars);
            auto dst_path = GetTmpPath() / "index.html";
            Save(dst_path, article, true);
        }


        path frontpage_path = GetTmpPath();
        frontpage_path /= GetArticlesPresentPageName(0);

        RenderRssForFrontpage(frontpage_path, vars);
    }

    float GetSitemapPriority(const string& key, float fixed = -1.0) {
        if (fixed >= 0.0) {
            return fixed;
        }
        float priority = options_.options.get<float>("seo.sitemap.priority."s + key,
                                                     50.0) / 100.0;
        return priority;
    }

    string GetArticlesPresentPageName(const int page, filesystem::path destName = {}) {

        if (destName.empty()) {
            destName = "index.html";
        }

        if (destName.string().find(GetTmpPath().string()) == 0) {
            destName = destName.string().substr(GetTmpPath().string().size());
            // If testName starts with a '/', remove it
            if (destName.has_root_directory()) {
                destName = destName.relative_path();
            }
        }

        if (page == 0) {
            return destName.string();
        }

        const auto fname = destName.stem().string();
        const auto ext = destName.extension().string();

        return format("{}_p{}{}", fname, page, ext);
    }

    void RenderRssForFrontpage(path path, std::map<std::string, std::string>& vars) {
        nodes_t rss_articles;
        int max_articles_in_rss_feed = options_.options.get("rss.max-articles", 64);
        for(auto& a: all_articles_) {
            if (FilterRss(*a->article)) {
                rss_articles.push_back(a->article);
            }
        }

        sort(rss_articles.begin(), rss_articles.end(),
             [](const auto& left, const auto& right) {
                 return left->GetMetadata()->published > right->GetMetadata()->published;
             });

        if (max_articles_in_rss_feed
            && (rss_articles.size() >= max_articles_in_rss_feed)) {
            rss_articles.resize(max_articles_in_rss_feed);
        }

        RenderRss(rss_articles, path, vars["site-title"],
                  vars["site-abstract"], vars["site-url"], vars["rss-abs"]);
    }

    bool FilterRss(const Node& article) {
        auto meta = article.GetMetadata();

        if (article.GetType() != Node::Type::ARTICLE) {
            LOG_TRACE << article << " is not not an article. Retracting from RSS feed";
            return false;
        }

        if (!meta->is_published) {
            LOG_TRACE << article << " is not in published state. Retracting from RSS feed";
            return false;
        }

        if (meta->type == "info") {
            LOG_TRACE << article << " has type info. Retracting from RSS feed";
            return false;
        }

        return true;
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

    string ListArticles(const RenderCtx& ctx, size_t num) override {
        auto articles = articles_for_frontpages_;
        // remove anythin that is not n->GetType() == Node::Type::ARTICLE
        articles.erase(remove_if(articles.begin(), articles.end(), [](const auto& a) {
            return a->GetType() != Node::Type::ARTICLE;
        }), articles.end());

        sort(articles.begin(), articles.end(),
             [](const auto& left, const auto& right) {
                 auto res = left->GetMetadata()->latestDate() - right->GetMetadata()->latestDate();
                 if (res) {
                     return res > 0;
                 }
                 return left->GetMetadata()->title > right->GetMetadata()->title;
             });

        articles.resize(min(num, articles.size()));
        map<string, string> vars;
        AssignDefauls(vars, ctx);

        for(const auto& n : articles) {
            const auto meta = n->GetMetadata();
            vars["article-type"] = boost::lexical_cast<string>(n->GetType());
            Assign(*meta, vars, ctx);
            string item = LoadTemplate("article-in-compact-list.html");
            ProcessTemplate(item, vars);
            vars["articles-in-list"] += item + "\n";
        }

        string section = LoadTemplate("compact-article-list.html");
        return ProcessTemplate(section, vars);
    }

    template <typename NodeListT>
    string RenderNodeList(const NodeListT& nodes,
                          const RenderCtx& ctx) {
        std::stringstream out;

        for(const auto& n : nodes) {
            const auto meta = n->GetMetadata();

            map<string, string> vars;
            AssignDefauls(vars, ctx);
            vars["article-type"] = boost::lexical_cast<string>(n->GetType());
            Assign(*meta, vars, ctx);

            if (n->GetType() == Node::Type::SERIES) {
                // Get the 3 newest articles in the series
                auto series = dynamic_pointer_cast<Series>(n);
                auto articles = series->GetArticles();
                Wash(articles);
                if (articles.empty()) {
                    LOG_WARN << "Series " << *n << " has no published articles!";
                    continue;
                }
                // Sort the list of articles, newest first
                sort(articles.begin(), articles.end(),
                     [](const auto& left, const auto& right) {
                         return left->GetMetadata()->latestDate() > right->GetMetadata()->latestDate();
                     });
                // Remove the oldest articles if there are more than 3
                if (articles.size() > 3) {
                    articles.resize(3);
                }

                for(const auto a : articles) {
                    map<string, string> avars;
                    AssignDefauls(avars, ctx);
                    avars["article-type"] = boost::lexical_cast<string>(a->GetType());
                    const auto ameta = a->GetMetadata();
                    Assign(*ameta, avars, ctx);
                    string item = LoadTemplate("article-in-series.html");
                    ProcessTemplate(item, avars);
                    vars["articles-in-series"] += item + "\n";
                }

                if (!articles.empty()) {
                    string item = LoadTemplate("latest-articles-header.html");
                    ProcessTemplate(item, vars);
                    vars["latest-articles-in-series"] = item;
                }
            }

            string item = LoadTemplate("article-in-list.html");
            ProcessTemplate(item, vars);
            out << item << endl;
        }

        return out.str();
    }

    std::optional<TagInfo> GetTag(const std::wstring& key) const {
        auto k = ToKey(key);
        if (auto t = tags_.find(k); t != tags_.end()) {
            return t->second;
        }
        return {};
    }

    template <typename TagList>
    string RenderTagList(const TagList& tags, const RenderCtx& ctx) {

        std::stringstream out;

        for(const auto& tag : tags) {
            map<string, string> vars;
            AssignDefauls(vars, ctx);

            // Only render tags that we know
            if (auto t = GetTag(tag)) {
                const auto& tag_info = *t;

                vars["url"] = ctx.GetRelativeUrl(tag_info.url);
                vars["name"] = stbl::ToString(tag);

                string tmplte = LoadTemplate("tag.html");
                ProcessTemplate(tmplte, vars);
                out << tmplte << endl;
            } else {
                LOG_WARN << "Tag " << stbl::ToString(tag) << " not found!";
            }
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

    string ToStringLocal(const time_t& when) {
        if (when) {
            static const string format = options_.options.get<string>("system.date.format", "%c");
            if (const auto tm = std::localtime(&when)) {
                return boost::lexical_cast<string>(put_time(tm, format.c_str()));
            }
        }
        return {};
    }

    string GetCurrentYear() {
        std::time_t now = std::time(nullptr);
        std::tm* tm = std::localtime(&now);

        std::ostringstream oss;
        oss << std::put_time(tm, "%Y");
        return oss.str();
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

        if (std::filesystem::is_regular_file(template_path)) {
            return Load(template_path);
        }

        auto it = embedded_templates_.find(name);
        if (it == embedded_templates_.end()) {
            throw runtime_error("Missing embedded template: "s + name);
        }

        return string(reinterpret_cast<const char *>(it->second.first), it->second.second);
    }

    boost::asio::awaitable<bool> SyntaxHighlight(string& content) {
        if (syntax_highlighter_.empty()) {
            co_return false;
        }

        static const boost::regex code_block(R"regex(<pre><code class="language-([a-zA-Z0-9+]{1,16})">(.*?)</code></pre>)regex",
                                             boost::regex::normal | boost::regex::icase);

        boost::smatch matches;
        size_t start_at = 0;
        while (boost::regex_search(content.cbegin() + start_at, content.cend(), matches, code_block)) {
            const auto offset = std::distance(content.cbegin(), matches[0].first);
            string language= matches[1];
            std::string code = matches[2];
            if (language == "c++" || language == "C++") {
                language = "cpp";
            }
            string highlighted = co_await SyntaxHighlightBlock(code, language);
            if (!highlighted.empty()) {
                content.replace(matches[0].first, matches[0].second, highlighted);
                start_at = offset + highlighted.size();
            } else {
                start_at = std::distance(content.cbegin(), matches[0].second);
            }
            assert(start_at > offset);
        }

        co_return true;
    }

    boost::asio::awaitable<string> SyntaxHighlightBlock(string part, const string& language) {
        string cmd = syntax_highlighter_;

        auto style = options_.options.get_optional<string>("chroma.style");
        if (!style) {
            style = "friendly";
        }

        boost::replace_all(part, "&amp;", "&");
        boost::replace_all(part, "&gt;", ">");
        boost::replace_all(part, "&lt;", "<");
        boost::replace_all(part, "&br;", "\n");
        boost::replace_all(part, "&quot;", "\"");

        vector<string> args;
        args.push_back("--html");
        args.push_back("--html-only");
        args.push_back("--html-inline-styles");
        args.push_back("--html-lines");
        args.push_back("--html-tab-width=4");
        // We need to handle section for each block to use thie
        //args.push_back("--html-linkable-lines");
        args.push_back("--filename=x." + string(language));
        args.push_back("--style=" + *style);

        try {
            auto ret = co_await stbl::popen(pipe_strand_, cmd, part, args);
            co_return ret;
        } catch (const std::exception& e) {
            LOG_ERROR << "Failed to syntax highlight: " << e.what();
            co_return part;
            //throw;
        }
    }

    nodes_t& Nodes() {
        return nodes_;
    }

    // All the nodes, including expired and not published ones
    nodes_t nodes_;

    // All articles that are published and not expired
    deque<shared_ptr<ArticleInfo>> all_articles_;
    deque<std::shared_ptr<Series>> all_series_;
    article_t index_; // Optional content for the frontpage

    // All articles and series that are to be listed on the front-page(s)
    deque<node_t> articles_for_frontpages_;

    // All tags from all content
    map<std::wstring, TagInfo> tags_;

    // Root menu item
    Menu menu_;

    path tmp_path__;

    const std::filesystem::path& GetTmpPath() const {
        return tmp_path__;
    }

    const time_t now_;
    unique_ptr<Scanner> scanner_;
    unique_ptr<ImageMgr> images_;
    const time_t roundup_;
    unique_ptr<Sitemap> sitemap_;
    std::string syntax_highlighter_;
    const size_t num_threads_{};/*{options_.threads == -1
                                  ? std::max<size_t>(std::thread::hardware_concurrency() - 1, 2)
                                  : static_cast<size_t>(std::max<int>(options_.threads, 1))}*/
    ba::thread_pool pool_{num_threads_};
    ba::strand<ba::thread_pool::executor_type> pipe_strand_{pool_.get_executor()};
};

const Options &ContentManager::GetOptions()
{
    return options_;
}

ContentManager &ContentManager::Instance()
{
    assert(self_);
    return *self_;
}

std::shared_ptr<ContentManager> ContentManager::Create(const Options& options)
{
    return make_shared<ContentManagerImpl>(options);
}

Options ContentManager::options_;



}

