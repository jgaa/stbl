
#include <memory>
#include <stack>
#include <assert.h>
#include <iostream>
#include <sstream>
#include <map>
#include <locale>
#include <codecvt>
#include <sstream>

#include <boost/filesystem.hpp>
#include <boost/algorithm/string.hpp>

#include "stbl/stbl.h"
#include "stbl/Series.h"
#include "stbl/Article.h"
#include "stbl/Content.h"
#include "stbl/logging.h"
#include "stbl/Page.h"
#include "stbl/Scanner.h"
#include "stbl/HeaderParser.h"
#include "stbl/utility.h"

using namespace std;
using namespace boost::filesystem;

namespace stbl {

class DirectoryScannerImpl : public Scanner
{
    struct Context
    {
        enum class Mode {
            GENERAL,
            SERIES
        };

        struct Location {
            Location(const vector<path>& argRecused, const path& argPath)
            : recursed{argRecused}, full_path{argPath} {}

            const vector<path> recursed;
            const path full_path;
            const bool is_index = false;
        };

        Context()
        : configuration{make_shared<std::vector<path>>()}
        , articles{make_shared<std::vector<Location>>()}
        {
        }

        Context(const Context&) = default;
        Context& operator = (const Context&) = default;

        void PrepareForSeries() {
            mode = Mode::SERIES;
            configuration = make_shared<std::vector<path>>();
            articles = make_shared<std::vector<Location>>();
            index.reset();
        }

        // Is at root-level
        bool IsRoot() const {
            return recursed.empty();
        }

        bool IsSeries() const {
            return mode == Mode::SERIES;
        }

        void SetIndex(const path& path) {
            if (!IsRoot() && !IsSeries()) {
                LOG_ERROR << "An index must be at the root level or in a series folder: "
                    << path;
                throw runtime_error("Found index.md out of context: "s + path.string());
            }

            LOG_DEBUG << "Adding " << path << " to context.";
            articles->push_back(Location(recursed, path));
        }

        Mode mode = Mode::GENERAL;
        vector<path> recursed;
        path current_path;
        shared_ptr<std::vector<path>> configuration;
        shared_ptr<std::vector<Location>> articles;
        shared_ptr<Location> index;
    };

public:
    DirectoryScannerImpl(const Options& options)
    : options_{options}
    {
        parser_ = HeaderParser::Create();
    }

    nodes_t Scan() override {
        path articles = options_.source_path;
        articles /= "articles";
        Context ctx;
        ScanDir(articles, ctx);
        Process(ctx);

        return move(nodes_);
    }

    void UpdateRequiredHeaders(const std::string & article,
                               const Node::Metadata& meta) override {

        LOG_INFO << "Updating headers in " << article;
        std::ifstream in(article);
        if (!in) {
            auto err = strerror(errno);
            LOG_ERROR << "IO error. Failed to open "
                << '"' << article << "\" for write: " << err;

            throw runtime_error("IO error");
        }

        // Make temporary file
        auto tmp_name = article + ".tmp";
        std::ofstream out(tmp_name);
        if (!out) {
            auto err = strerror(errno);
            LOG_ERROR << "IO error. Failed to open "
                << '"' << tmp_name << "\" for write: " << err;

            throw runtime_error("IO error");
        }

        // Write header
        out << "---" << endl;
        WriteIf(out, "uuid", meta.uuid);
        if (meta.have_title) {
            WriteIf(out, "title", meta.title);
        }
        WriteIf(out, "abstract", meta.abstract);
        WriteIf(out, "menu", meta.menu);
        WriteIf(out, "template", meta.tmplte);
        WriteIf(out, "type", meta.type);
        WriteIf(out, "tags", meta.tags);
        if (meta.have_updated) {
            WriteIf(out, "updated", meta.updated);
        }
        WriteIf(out, "published", meta.published);
        WriteIf(out, "expires", meta.expires);
        WriteIf(out, "banner", meta.banner);

        out << "---" << endl;

        // Copy content
        EatHeader(in);
        copy(istreambuf_iterator<char>(in), istreambuf_iterator<char>(),
             ostreambuf_iterator<char>(out));

        in.close();
        out.close();

        // Set file date
        auto when = boost::filesystem::last_write_time(article);
        boost::filesystem::last_write_time(tmp_name, when);

        // Rename
        boost::filesystem::remove(article);
        boost::filesystem::rename(tmp_name, article);
    }

private:
    void WriteIf(ostream& out, const char *name, const std::string& value) {
        if (!value.empty()) {
            out << name << ": " << value << endl;
        }
    }

    void WriteIf(ostream& out, const char *name, const std::wstring& value) {
        WriteIf(out, name, ToString(value));
    }

    void WriteIf(ostream& out, const char *name, std::vector<std::wstring> value) {
        if (!value.empty()) {
            bool virgin = true;
            out << name << ": ";

            for(auto& v : value) {
                if (virgin) {
                    virgin = false;
                } else {
                    out << ", ";
                }

                out << ToString(v);
            }

            out << endl;
        }
    }

    void WriteIf(ostream& out, const char *name, const time_t& value) {
        if (value) {
            WriteIf(out, name, ToStringAnsi(value));
        }
    }

    void ScanDir(const path& path, Context ctx) {
        if (!is_directory(path)) {
            LOG_ERROR << path << " is not a directory!";
            throw std::runtime_error("Can only scan existing directories.");
        }

        for(auto entry : directory_iterator(path)) {
            LOG_TRACE << "Examining " << entry.path();

            const auto name = entry.path().filename().string();

            if (is_directory(entry.path())) {
                const auto subdir = entry.path();

                if (name.at(0) == '_') {
                    // Directory name starts with underscore. Just scan.
                    auto subCtx = Recurse(subdir, ctx);
                    ScanDir(subdir, subCtx);
                } else {
                    // Series folder.

                    if (ctx.IsSeries()) {
                        LOG_ERROR
                            << "Already building a series when examining "
                            << subdir;
                        throw std::runtime_error("Recursive series are not supported.");
                    }

                    auto subCtx = Recurse(subdir, ctx);
                    subCtx.PrepareForSeries();
                    LOG_DEBUG << "Building series: " << name;
                    ScanDir(subdir, subCtx);
                    Process(subCtx);
                    LOG_DEBUG << "Done with series: " << name;
                }

            } else if (is_regular_file(entry.path())) {
                if (entry.path().filename().string() == "index.md"s) {
                    ctx.SetIndex(entry.path());
                    continue;
                }

                const auto ext = entry.path().extension();
                if (ext == ".md") {
                    LOG_DEBUG << "Adding article: " << name;
                    ctx.articles->push_back(Context::Location(ctx.recursed, entry.path()));

                } else if (ext == ".conf") {
                    if (ctx.IsSeries()) {
                        LOG_DEBUG << "Adding configuration: " << entry.path();
                        ctx.configuration->push_back(entry.path());
                    } else {
                        LOG_WARN << "Ignoring " << entry.path()
                            << " outside series.";
                    }
                } else {
                    LOG_WARN << "Ignoring file with unsupported extension "
                        << " (" << entry.path().extension() << "): "
                        << entry.path();
                }

            } else {
                LOG_WARN << "Skipping [non-recognizable type] entry: "
                    << entry.path();
            }
        }
    }

    Context Recurse(const path subdir, const Context& ctx) {
        Context newCtx = ctx;
        newCtx.recursed.push_back(subdir);

        if (find(ctx.recursed.begin(), ctx.recursed.end(), subdir)
            != ctx.recursed.end()) {
            LOG_ERROR << "Detected recursive loop in directory structure:";
            for(const auto p: newCtx.recursed) {
                LOG_ERROR << "   " << p.string();
            }

            throw std::runtime_error("Recursive loop in directory structure.");
        }

        newCtx.current_path = subdir;
        return newCtx;
    }

    void Process(const Context& ctx) {
        if (ctx.mode == Context::Mode::SERIES) {
            nodes_.push_back(ProcessSeries(ctx));
        } else {
            auto articles = ProcessArticles(ctx);
            nodes_.insert(nodes_.end(), articles.begin(), articles.end());
        }
    }

    std::shared_ptr<Series> ProcessSeries(const Context& ctx) {
        auto series = Series::Create();
        // TODO: Look for a "magic" file-name that contains a first-page for the series

        // Deal with configuration

        // Set the properties for the series
        //    - Name
        //    - Last updated time (based on the newest article)
        auto md = make_shared<Node::Metadata>();

        if (md->title.empty()) {
            md->title = GetTitleFromPath(ctx.current_path);
        }

        if (md->article_path_part.empty()) {
            md->article_path_part = ctx.current_path.stem().string();
        }

        if (!md->published && md->is_published) {
                    md->published = GetTimeFromPath(ctx.current_path);
        }

        if (!md->updated && md->is_published) {
            md->updated = GetTimeFromPath(ctx.current_path);
        }

        series->SetMetadata(md);

        // Add articles
        auto articles = ProcessArticles(ctx, series);
        series->AddArticles(move(articles));
        return move(series);
    }

    articles_t ProcessArticles(const Context& ctx, serie_t series = {}) {
        articles_t articles;

        for(const auto& a : *ctx.articles) {
            auto article = Article::Create();

            try {
                auto hdr = make_shared<Article::Header>();
                ParseHeader(*hdr, FetchHeader(a.full_path));

                if (a.full_path.filename() == "index.md") {
                    hdr->type = "index"s;
                    hdr->tags.clear();
                } else {

                    if (hdr->title.empty()) {
                        hdr->title = GetTitleFromPath(a.full_path);
                    }

                    if (!hdr->published && hdr->is_published) {
                        hdr->published = GetTimeFromPath(a.full_path);
                    }

                    if (!hdr->updated && hdr->is_published) {
                        hdr->updated = GetTimeFromPath(a.full_path);
                    }

                    if (hdr->article_path_part.empty()) {
                        hdr->article_path_part = GetPath(ctx, a);
                    }

                    article->SetAuthors(hdr->authors);

                    if (series) {
                        article->SetSeries(series);
                    }
                }

                article->SetMetadata(hdr);

                auto content = Content::Create(a.full_path);
                content->AddPage(Page::Create(a.full_path));
                article->SetContent(move(content));

                articles.push_back(article);

            } catch(exception& ex) {
                LOG_ERROR << "Generation failed processing article: " << a.full_path;
                throw;
            }
        }

        return move(articles);
    }


    std::string GetPath(const Context& ctx, const Context::Location& location) {
        switch(options_.path_layout) {
            case Options::PathLayout::SIMPLE:
                return location.full_path.stem().string();
            case Options::PathLayout::RECURSIVE:
            {
                path where;
                for(const auto p: location.recursed) {
                    auto filename = p.filename().string();
                    if (!filename.empty() && (filename[0] == '_')) {
                        filename = filename.substr(1);
                    }
                    where /= filename;
                }
                where /= location.full_path.stem().string();
                return where.string();
            }
            default:
                assert(false && "Unknown layout");
        }
    }

    std::wstring GetTitleFromPath(const path& path) {
        auto name = path.stem().string();
        boost::replace_all(name, "_", " ");
        if(!name.empty()) {
            locale loc;
            name[0] = toupper(name[0], loc);
        }

        return converter.from_bytes(name);
    }

    time_t GetTimeFromPath(const path& path) {
        auto when = last_write_time(path);
        return when;
    }

    /* Do it simple. Read only until we have the header.
     *
     * (Actually, I don't know how to ask boost::spirit to parse just
     * the header section. This approach will only read the part of the
     * file that we need for now.)
     */
    std::string FetchHeader(path path) {
        ostringstream out;

        string inpath = path.string();
        std::ifstream in(inpath.c_str());
        array<char, 1024> buffer;
        int delimiters = 0;

        while(in) {
            in.getline(buffer.data(), buffer.size());
            const auto len = in.gcount();

            bool is_delimiter = false;
            if (len >=3) {
                if ((buffer[0] == '-')
                    && (buffer[0] == '-')
                    &&  (buffer[0] == '-')) {
                    ++delimiters;
                    is_delimiter = true;
                }
            }

            if (delimiters == 1) {
                if (!is_delimiter) {
                    out << buffer.data() << '\n';
                }
            } else if (delimiters == 2) {
                return out.str();
            }
        }

        LOG_ERROR << "Failed to extract header-section from " << path;
        throw runtime_error("Parse error");
    }

    void ParseHeader(Article::Header& header, std::string input) {
        parser_->Parse(header, input);
    }

    const Options& options_;
    nodes_t nodes_;
    unique_ptr<HeaderParser> parser_;
    std::wstring_convert<std::codecvt_utf8_utf16<wchar_t>> converter;
};


std::unique_ptr<Scanner> Scanner::Create(const Options& options)
{
    return make_unique<DirectoryScannerImpl>(options);
}

}
