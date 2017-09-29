
#include <memory>
#include <stack>
#include <assert.h>
#include <iostream>
#include <sstream>
#include <map>

#include <boost/filesystem.hpp>

#include "stbl/stbl.h"
#include "stbl/Series.h"
#include "stbl/Article.h"
#include "stbl/logging.h"
#include "stbl/Scanner.h"
#include "stbl/HeaderParser.h"

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

        Context()
        : configuration{make_shared<std::vector<path>>()}
        , articles{make_shared<std::vector<path>>()}
        {
        }

        Context(const Context&) = default;
        Context& operator = (const Context&) = default;

        void PrepareForSeries() {
            mode = Mode::SERIES;
            configuration = make_shared<std::vector<path>>();
            articles = make_shared<std::vector<path>>();
        }

        Mode mode = Mode::GENERAL;
        vector<path> recursed;
        std::shared_ptr<std::vector<path>> configuration;
        std::shared_ptr<std::vector<path>> articles;
    };

public:
    DirectoryScannerImpl(const Options& options)
    : options_{options}
    {
    }

    nodes_t Scan() override {
        path articles = options_.source_path;
        articles /= "articles";
        Context ctx;
        ScanDir(articles, ctx);
        return Process(ctx);
    }

private:
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

                    if (ctx.mode == Context::Mode::SERIES) {
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
                const auto ext = entry.path().extension();
                if (ext == ".md") {
                    LOG_DEBUG << "Adding article: " << name;
                    ctx.articles->push_back(entry.path());

                } else if (ext == ".conf") {
                    if (ctx.mode == Context::Mode::SERIES) {
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

        return newCtx;
    }

    nodes_t Process(const Context& ctx) {
        nodes_t nodes;

        if (ctx.mode == Context::Mode::SERIES) {
            nodes.push_back(ProcessSeries(ctx));
        } else {
            auto articles = ProcessArticles(ctx);
            nodes.insert(nodes.end(), articles.begin(), articles.end());
        }

        return nodes;
    }

    std::shared_ptr<Series> ProcessSeries(const Context& ctx) {
        auto series = Series::Create();
        // TODO: Look for a "magic" file-name that contains a first-page for the series

        // Deal with configuration

        // Set the properties for the series
        //    - Name
        //    - Last updated time (based on the newest article)

        // Add articles
        auto articles = ProcessArticles(ctx);

    }

    articles_t ProcessArticles(const Context& ctx) {
        articles_t articles;

        for(const auto& a : *ctx.articles) {
            auto article = Article::Create();
            auto md = make_shared<Node::Metadata>();

            try {
                auto hdr = make_shared<Article::Header>();
                ParseHeader(*hdr, FetchHeader(a));

                article->SetMetadata(hdr);
                article->SetAuthors(hdr->authors);
                articles.push_back(article);

            } catch(exception& ex) {
                LOG_ERROR << "Generation failed processing article: " << a;
                throw;
            }
        }

        return articles;
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

        LOG_WARN << "Failed to extract header-section from " << path;
        throw runtime_error("Parse error");
    }

    void ParseHeader(Article::Header& header, std::string input) {
        auto parser = HeaderParser::Create();
        parser->Parse(header, input);
    }

    const Options& options_;
    nodes_t nodes_;
};


std::unique_ptr<Scanner> Scanner::Create(const Options& options)
{
    return make_unique<DirectoryScannerImpl>(options);
}

}
