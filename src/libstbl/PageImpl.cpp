
#include <string.h>
#include <fstream>
#include <regex>
#include <sstream>

#include <boost/algorithm/string/replace.hpp>
#include <boost/regex.hpp>

#include "cmark-gfm.h"

#include "stbl/stbl.h"
#include "stbl/Page.h"
#include "stbl/logging.h"
#include "stbl/utility.h"
#include "stbl/ContentManager.h"

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

    size_t Render2Html(std::ostream & out, RenderCtx& ctx) override {

        if (!path_.empty()) {
            ifstream in(path_.string());
            if (!in) {
                auto err = strerror(errno);
                LOG_ERROR << "IO error. Failed to open "
                    << path_ << ": " << err;

                throw runtime_error("IO error");
            }

            return Render2Html(in, out, ctx);
        }

        std::istringstream in{content_};
        return Render2Html(in, out, ctx);
    }

private:
    size_t Render2Html(istream& in, ostream& out, RenderCtx& ctx) {
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

        handleVideo(content, ctx);

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
            return words;
        }
        LOG_ERROR << "Failed to convert markdown to HTML";
        out << content;
        return words;
    }

    enum class Scaling {
        p360 = 360,
        p480 = 480,
        p720 = 720,
        p1080 = 1080,
        p1440 = 1440,
        p2160 = 2160
    };

    std::vector<std::string>
    convertVideo(const std::filesystem::path& inputFilePath, const std::string& prefix,  Scaling scaling) {
        if (!fs::exists(inputFilePath)) {
            LOG_ERROR << "Input file does not exist: " << inputFilePath;
            return {};
        }

        const int height = static_cast<int>(scaling);
        const auto filename = inputFilePath.stem().string();
        const auto parentPath = inputFilePath.parent_path();
        const auto scale_tag = "_p" + std::to_string(height);

        const auto output_mp4 = parentPath / "_mp4" / (filename + scale_tag + ".mp4");
        const auto output_webm = parentPath / "_webm" /  (filename + scale_tag + ".webm");
        const auto output_ogv = parentPath / "_ogv" / (filename + scale_tag + ".ogv");

        const string scale_filter = "scale=-2:" + std::to_string(height);

        const string cmd_mp4 = "ffmpeg -i " + inputFilePath.string() + " -vf \"" + scale_filter +
                               "\" -c:v libx264 -crf 23 -preset medium -c:a aac -b:a 128k " + output_mp4.string();
        const string cmd_webm = "ffmpeg -i " + inputFilePath.string() + " -vf \"" + scale_filter +
                                "\" -c:v libvpx-vp9 -b:v 0 -crf 31 -c:a libvorbis " + output_webm.string();
        const string cmd_ogv = "ffmpeg -i " + inputFilePath.string() + " -vf \"" + scale_filter +
                               "\" -c:v libtheora -q:v 7 -c:a libvorbis -q:a 5 " + output_ogv.string();

        if (!fs::exists(output_mp4)) {
            LOG_DEBUG << "Executing: " << cmd_mp4;
            CreateDirectoryForFile(output_mp4);
            std::system(cmd_mp4.c_str());
        }

        if (!fs::exists(output_webm)) {
            LOG_DEBUG << "Executing: " << cmd_webm;
            CreateDirectoryForFile(output_webm);
            std::system(cmd_webm.c_str());
        }

        if (!fs::exists(output_ogv)) {
            LOG_DEBUG << "Executing: " << cmd_ogv;
            CreateDirectoryForFile(output_ogv);
            std::system(cmd_ogv.c_str());
        }

        // We want the path from "video/" for the output file
        auto relative_path = [](const fs::path& path) {
            auto pparent = path.parent_path().parent_path();
            auto parent = path.parent_path();
            auto filename = path.filename();

            fs::path path_relative;
            path_relative /= pparent / parent.filename() / filename;

            return path_relative;
        };

        vector<string> result;
        result.emplace_back("<source src=\""s + prefix + relative_path(output_webm).string() + "\" type=\"video/webm\">");
        result.emplace_back("<source src=\""s + prefix + relative_path(output_mp4).string() + "\" type=\"video/mp4\">");
        result.emplace_back("<source src=\""s + prefix + relative_path(output_ogv).string() + "\" type=\"video/ogg\">");

        return result;
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

    void handleVideo(std::string& content, RenderCtx& ctx)
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

            const auto sources = convertVideo(source, ctx.getRelativePrefix(), toScaling(scaling));

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

