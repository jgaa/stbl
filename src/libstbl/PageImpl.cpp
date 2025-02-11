
#include <string.h>
#include <fstream>
#include <regex>
#include <sstream>

#include <boost/algorithm/string/replace.hpp>
#include <boost/regex.hpp>
#include <boost/asio/experimental/awaitable_operators.hpp>

#include "cmark-gfm.h"

#include "stbl/stbl.h"
#include "stbl/Page.h"
#include "stbl/logging.h"
#include "stbl/utility.h"
#include "stbl/ContentManager.h"
#include "stbl/pipe.h"

using namespace std;
using namespace std::string_literals;
namespace fs = std::filesystem;

namespace stbl {


class PageImpl : public Page
{
public:
    PageImpl(const std::filesystem::path& path)
    : path_{path}, content_{}
    {
    }

     PageImpl(const string& content)
    : path_{}, content_{content}
    {
    }

    ~PageImpl()  {
    }

    boost::asio::awaitable<size_t> Render2Html(std::ostream & out, RenderCtx& ctx) override {

        if (!path_.empty()) {
            ifstream in(path_.string());
            if (!in) {
                auto err = strerror(errno);
                LOG_ERROR << "IO error. Failed to open "
                    << path_ << ": " << err;

                throw runtime_error("IO error");
            }

            co_return co_await Render2Html(in, out, ctx);
        }

        std::istringstream in{content_};
        co_return co_await Render2Html(in, out, ctx);
    }

private:
    boost::asio::awaitable<size_t> Render2Html(istream& in, ostream& out, RenderCtx& ctx) {
        EatHeader(in);
        string content((std::istreambuf_iterator<char>(in)),
                       istreambuf_iterator<char>());

        size_t words = 0;
        static regex word_pattern("\\w+");
        sregex_iterator next(content.begin(), content.end(), word_pattern);
        sregex_iterator end;
        for (; next != end; ++next) {
            ++words;
        }

        co_await handleVideo(content, ctx);

        // Quick hack to handle images in series.
        static const std::regex images{R"(.*(!\[.+\])\((images\/.+)\))"};
        content = std::regex_replace(content, images, "$1("s + ctx.getRelativePrefix() + "$2)");

        // Process markdown
        if (char * output{cmark_markdown_to_html(content.c_str(), content.size(),
            CMARK_OPT_DEFAULT | CMARK_OPT_VALIDATE_UTF8 | CMARK_OPT_UNSAFE)}) {
            auto deleter = [](void *ptr) {
                // We are using a C library, so call free()
                if (ptr) free(ptr);
            };
            unique_ptr<char, decltype(deleter)> output_ptr{output, deleter};
            string_view output_w{output_ptr.get()};

            content.clear();
            out << output_w;
            co_return words;
        }
        LOG_ERROR << "Failed to convert markdown to HTML";
        out << content;
        co_return words;
    }

    enum class Scaling {
        p360 = 360,
        p480 = 480,
        p720 = 720,
        p1080 = 1080,
        p1440 = 1440,
        p2160 = 2160
    };

    boost::asio::awaitable<std::vector<std::string>>
    convertVideo(const std::filesystem::path& inputFilePath, const std::string& prefix,  Scaling scaling) {
        vector<string> result;
        if (!fs::exists(inputFilePath)) {
            LOG_ERROR << "Input file does not exist: " << inputFilePath;
            co_return result;
        }

        const int height = static_cast<int>(scaling);
        const auto filename = inputFilePath.stem().string();
        const auto parentPath = inputFilePath.parent_path();
        const auto scale_tag = "_p" + std::to_string(height);

        const auto output_mp4 = parentPath / "_mp4" / (filename + scale_tag + ".mp4");
        const auto output_webm = parentPath / "_webm" /  (filename + scale_tag + ".webm");
        const auto output_ogv = parentPath / "_ogv" / (filename + scale_tag + ".ogv");

        const string scale_filter = "scale=-2:" + std::to_string(height);


        if (!fs::exists(output_mp4)) {
            vector<string> args;
            args.push_back("-loglevel");
            args.push_back("error");
            args.push_back("-i");
            args.push_back(inputFilePath.string());
            args.push_back("-vf");
            args.push_back(scale_filter);
            args.push_back("-c:v");
            args.push_back("libx264");
            args.push_back("-crf");
            args.push_back("23");
            args.push_back("-preset");
            args.push_back("medium");
            args.push_back("-c:a");
            args.push_back("aac");
            args.push_back("-b:a");
            args.push_back("128k");
            args.push_back(output_mp4.string());

            //LOG_DEBUG << "Executing: " << cmd_mp4;
            CreateDirectoryForFile(output_mp4);
            co_await run("ffmpeg", args);
        }

        if (!fs::exists(output_webm)) {
            vector<string> args;
            args.push_back("-loglevel");
            args.push_back("error");
            args.push_back("-i");
            args.push_back(inputFilePath.string());
            args.push_back("-vf");
            args.push_back(scale_filter);
            args.push_back("-c:v");
            args.push_back("libvpx-vp9");
            args.push_back("-b:v");
            args.push_back("0");
            args.push_back("-crf");
            args.push_back("31");
            args.push_back("-c:a");
            args.push_back("libvorbis");
            args.push_back(output_webm.string());

            CreateDirectoryForFile(output_webm);
            co_await run("ffmpeg", args);
        }

        if (!fs::exists(output_ogv)) {
            vector<string> args;
            args.push_back("-loglevel");
            args.push_back("error");
            args.push_back("-i");
            args.push_back(inputFilePath.string());
            args.push_back("-vf");
            args.push_back(scale_filter);
            args.push_back("-c:v");
            args.push_back("libtheora");
            args.push_back("-q:v");
            args.push_back("7");
            args.push_back("-c:a");
            args.push_back("libvorbis");
            args.push_back("-q:a");
            args.push_back("5");
            args.push_back(output_ogv.string());

            CreateDirectoryForFile(output_ogv);
            co_await run("ffmpeg", args);
        }

        // We want the path from "video/" for the output file
        auto relative_path = [](const fs::path& path) {
            auto pparent = path.parent_path().parent_path().filename();
            auto parent = path.parent_path().filename();
            auto filename = path.filename();

            fs::path path_relative;
            path_relative /= pparent / parent.filename() / filename;

            return path_relative;
        };

        result.emplace_back("<source src=\""s + prefix + relative_path(output_webm).string() + "\" type=\"video/webm\">");
        result.emplace_back("<source src=\""s + prefix + relative_path(output_mp4).string() + "\" type=\"video/mp4\">");
        result.emplace_back("<source src=\""s + prefix + relative_path(output_ogv).string() + "\" type=\"video/ogg\">");

        co_return result;
    }

    Scaling toScaling(std::string_view name) {
        if (name == "p360")
            return Scaling::p360;
        if (name == "p480")
            return Scaling::p480;
        if (name == "p720")
            return Scaling::p720;
        if (name == "p1080")
            return Scaling::p1080;
        if (name == "p1440")
            return Scaling::p1440;
        if (name == "p2160")
            return Scaling::p2160;
        return Scaling::p720;
    }

    boost::asio::awaitable<void> handleVideo(std::string& content, RenderCtx& ctx)
    {
        static const boost::regex video_pat{R"(!\[(.*?)\]\((video\/([a-zA-Z0-9\-_\.]+))(;(p\d+))?\))",
                                            boost::regex::normal | boost::regex::icase};
        boost::smatch matches;
        size_t start_at = 0;
        while (boost::regex_search(content.cbegin() + start_at, content.cend(), matches, video_pat)) {
            const auto offset = std::distance(content.cbegin(), matches[0].first);

            const string label = matches[1];
            const string source = matches[2];
            const string scaling = matches[5];

            fs::path full_video_path = ContentManager::GetOptions().source_path;
            full_video_path /= source;

            const auto sources = co_await convertVideo(full_video_path, ctx.getRelativePrefix(), toScaling(scaling));

            string video_tag = "<video controls>\n";
            for(const auto& src: sources) {
                video_tag += src + "\n";
            }
            video_tag += "Your browser does not support the video tag\n</video>";

            if (!video_tag.empty()) {
                content.replace(matches[0].first, matches[0].second, video_tag);
                start_at = offset + video_tag.size();
            } else {
                start_at = std::distance(content.cbegin(), matches[0].second);
            }
            assert(start_at > offset);
        }
    }

    const std::filesystem::path path_;
    const std::string content_;
};

page_t Page::Create(const std::filesystem::path& path) {
    return make_shared<PageImpl>(path);
}

page_t Page::Create(const string& content) {
    return make_shared<PageImpl>(content);
}

}

