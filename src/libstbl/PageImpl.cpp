
#include <string.h>
#include <fstream>
#include <regex>
#include <sstream>
#include <array>
#include <ranges>
#include <algorithm>

#include <boost/algorithm/string/replace.hpp>
#include <boost/regex.hpp>
#include <boost/asio/experimental/awaitable_operators.hpp>
#include <boost/process.hpp>
#include <boost/process/v1/child.hpp>
#include <boost/process/v1/io.hpp>
#include <boost/json.hpp>

#include "cmark-gfm.h"

#include "stbl/stbl.h"
#include "stbl/Page.h"
#include "stbl/logging.h"
#include "stbl/utility.h"
#include "stbl/ContentManager.h"
#include "stbl/pipe.h"
#include "stbl/ImageMgr.h"

using namespace std;
using namespace std::string_literals;
namespace fs = std::filesystem;
namespace bp = boost::process;
namespace json = boost::json;

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


    bool containsVideo() const noexcept  override {
        return using_video_;
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

        co_await handleResponsiveImage(content, ctx);
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

    struct Aspect {
        double ratio;    // width/height
        bool landscape;  // true if w>=h
    };

    Aspect computeAspect(int w, int h) {
        return { double(w) / double(h), w >= h };
    }

    // naturalWidth is the CSS width (in px) of a video scaled to `targetHeight`,
    // taking into account the real aspect ratio.
    int naturalWidthForHeight(double aspectRatio, int targetHeight) {
        return int(std::round(aspectRatio * targetHeight));
    }

    static constexpr std::array<Scaling, 6> all_video_scalings {
            Scaling::p360, Scaling::p480, Scaling::p720,
            Scaling::p1080, Scaling::p1440, Scaling::p2160
    };

    struct Dimensions { int width, height; };

    Dimensions probeDimensions(const fs::path& file) {
        // Prepare an IP stream to capture stdout
        bp::v1::ipstream out;

        // Spawn ffprobe, redirecting stdout→out, stderr→null
        bp::v1::child c(
            bp::v1::search_path("ffprobe"),
            "-v", "error",
            "-select_streams", "v:0",
            "-show_entries", "stream=width,height",
            "-of", "csv=p=0",
            file.string(),
            bp::v1::std_out > out,
            bp::v1::std_err > bp::v1::null
            );

        // Read the single line: "WIDTH,HEIGHT"
        std::string line;
        if (!std::getline(out, line)) {
            c.wait();
            throw std::runtime_error("ffprobe produced no output for " + file.string());
        }

        // Wait and check exit status
        c.wait();
        if (c.exit_code() != 0) {
            throw std::runtime_error("ffprobe exited with code " +
                                     std::to_string(c.exit_code()));
        }

        // Parse dimensions
        std::istringstream iss(line);
        int w, h;
        char comma;
        if (!(iss >> w >> comma >> h) || comma != ',') {
            throw std::runtime_error("Unexpected ffprobe output: " + line);
        }

        return { w, h };
    }

    struct Rendition {
        std::string mediaQuery, url, mimeType;
        Scaling scale;
    };
    struct VideoRenditions {
        std::string posterUrl;
        std::vector<Rendition> sources;
        Dimensions dim;
    };

    fs::path buildPosterPath(const fs::path& path) {
        // Poster is stored in the same directory as the video, but with a different name
        return path.parent_path() / "_poster_" / (path.stem().string() + ".jpg");
    }

    fs::path buildRenditionPath(const fs::path& path, Scaling scaling, const std::string& ext) {
        // Rendition is stored in the same directory as the video, but with a different name
        fs::path r = path.parent_path();
        r /= "_scale_" + std::to_string(static_cast<int>(scaling));
        fs::create_directories(r); // Ensure the directory exists
        // Use the original filename stem and append the scaling and extension
        r /= (path.stem().string() + "." + ext);
        LOG_TRACE_N << "Rendition path for " << path << " --> " << r;
        return r;
    }

    std::string buildMediaQuery(Scaling s, const Aspect& a) {
        // compute the “cutoff” widths for this scaling level
        int h = static_cast<int>(s);
        int w  = naturalWidthForHeight(a.ratio, h);

        // We’ll build mutually exclusive ranges in ascending order:
        switch (s) {
        case Scaling::p360: {
            return "(max-width: " + std::to_string(w) + "px)";
        }
        case Scaling::p480: {
            int prevW = naturalWidthForHeight(a.ratio, int(Scaling::p360));
            return "(min-width: " + std::to_string(prevW + 1) + "px)"
                                                                " and (max-width: " + std::to_string(w) + "px)";
        }
        case Scaling::p720: {
            int prevW = naturalWidthForHeight(a.ratio, int(Scaling::p480));
            return "(min-width: " + std::to_string(prevW + 1) + "px)"
                                                                " and (max-width: " + std::to_string(w) + "px)";
        }
        case Scaling::p1080: {
            int prevW = naturalWidthForHeight(a.ratio, int(Scaling::p720));
            return "(min-width: " + std::to_string(prevW + 1) + "px)"
                                                                " and (max-width: " + std::to_string(w) + "px)";
        }
        case Scaling::p1440: {
            int prevW = naturalWidthForHeight(a.ratio, int(Scaling::p1080));
            return "(min-width: " + std::to_string(prevW + 1) + "px)"
                                                                " and (max-width: " + std::to_string(w) + "px)";
        }
        case Scaling::p2160: {
            int prevW = naturalWidthForHeight(a.ratio, int(Scaling::p1440));
            return "(min-width: " + std::to_string(prevW + 1) + "px)";
        }
        }
        return "";
    }

    boost::asio::awaitable<VideoRenditions>
    generateRenditions(
        const fs::path& input,
        const std::string& urlPrefix,
        Scaling startLevel) {
        VideoRenditions out;
        out.dim = probeDimensions(input);
        auto aspect= computeAspect(out.dim.width, out.dim.height);

        if (!std::filesystem::exists(input)) {
            LOG_ERROR << "Video does not exist: " << input;
            co_return VideoRenditions{}; // Return empty if the input file does not exist
        }

        const auto updated_time = std::filesystem::last_write_time(input);

        // We want the path from "video/" for the output file
        auto relativePath = [](const fs::path& path) {
            auto pparent = path.parent_path().parent_path().filename();
            auto parent = path.parent_path().filename();
            auto filename = path.filename();

            fs::path path_relative;
            path_relative /= pparent / parent.filename() / filename;

            return path_relative;
        };

        // 2a) Poster frame @ 3sec at small thumbnail size
        fs::path poster = buildPosterPath(input);
        CreateDirectoryForFile(poster);
        if (!fileExists(poster, updated_time)) {
            const string poster_out = poster.string();
            const string poster_in = input.string();
            array<string, 11> args{
                "-loglevel", "error", "-i", poster_in,
                "-ss", "3", "-vframes", "1",
                "-vf", (out.dim.width >= out.dim.height
                        ? "scale=-2:360"
                        : "scale=360:-2"),
                poster_out
            };

            co_await run("ffmpeg", args);
        }
        out.posterUrl = urlPrefix + relativePath(poster).string();

        // 2b) For each scaling ≥ startLevel generate an MP4 (and optionally WebM)
        bool landscape = out.dim.width >= out.dim.height;
        for (auto s : all_video_scalings) {
            if (s > startLevel) {
                continue;
            }
            int target  = static_cast<int>(s);
            std::string filter = landscape
                                     ? "scale=-2:" + std::to_string(target)
                                     : "scale=" + std::to_string(target) + ":-2";

            fs::path mp4 = buildRenditionPath(input, s, "mp4");
            CreateDirectoryForFile(mp4);
            if (!fileExists(mp4, updated_time)) {
                LOG_INFO_N << "Converting video to " << mp4 << " with scaling " << target;
                const array<string, 17> args = {
                    "-loglevel","error","-i",input.string(),
                    "-vf", filter,
                    "-c:v","libx264","-crf","21","-preset","slow",
                    "-c:a","aac","-b:a","128k", mp4.string()
                };

                co_await run("ffmpeg", args);
            }
            out.sources.emplace_back(
                /* media= */ buildMediaQuery(s, aspect),
                /* url=   */ urlPrefix + relativePath(mp4).string(),
                /* type=  */ "video/mp4",
                /* scale= */ s
            );
        }

        // Sort out.sources so that the largest resolution is first
        std::sort(out.sources.begin(), out.sources.end(),
                  [](const Rendition& a, const Rendition& b) {
                      return a.scale < b.scale;
                  });

        // remove "and (max-width: ...px)" from the last source
        if (!out.sources.empty()) {
            Rendition& last = out.sources.back();
            if (last.mediaQuery.starts_with("(min-width: ")) {
                // Remove the "and (max-width: ...px)" part
                size_t pos = last.mediaQuery.find(" and ");
                if (pos != std::string::npos) {
                    last.mediaQuery.erase(pos);
                }
            }
        }

        co_return out;
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

    boost::asio::awaitable<void>
    handleVideo(std::string& content, RenderCtx& ctx)
    {
        static const boost::regex pat{R"(!\[(.*?)\]\((video\/([A-Za-z0-9\-_\.]+))(;(p\d+))?\))",
                                      boost::regex::normal | boost::regex::icase};

        boost::smatch m;
        std::vector<std::tuple<std::size_t, std::size_t, std::string>> ops;
        auto scan_start = content.cbegin();

        auto video_num = 0u;

        // 1) find all matches (record their start/length + replacement)
        while (boost::regex_search(scan_start, content.cend(), m, pat)) {
            // absolute byte-offset in the *original* buffer
            std::size_t pos  = m.position(size_t{0}) + std::distance(content.cbegin(), scan_start);
            std::size_t len  = m.length(0);

            fs::path input_path = ContentManager::GetOptions().source_path;
            input_path /= string{m[2]};
            const auto start_level = toScaling(string{m[5]});
            const auto rend = co_await generateRenditions(input_path, ctx.getRelativePrefix(), start_level);
            std::ostringstream tag;
            tag << format("<video id=\"videoplayer_{}\" controls preload=\"metadata\"", video_num)
                << " poster=\"" << rend.posterUrl << "\""
                << " playsinline"
                << " style=\"max-width:100%; max-height:80vh; height:auto; width:auto; display:block;\""
                << ">\n";

            for (auto& r : rend.sources) {
                tag << "  <source media=\"" << r.mediaQuery
                    << "\" src=\"" << r.url
                    << "\" size=\"" << int(r.scale) << ' '
                    << "\" type=\"" << r.mimeType << "\">" << "\n";
            }

            tag << "  Your browser doesn’t support HTML5 video — "
                   "<a href=\"" << rend.sources.back().url << "\">download it</a>."
                << "\n</video>";

            ops.emplace_back(pos, len, tag.str());

            // advance scan_start past *this* match in the original buffer
            scan_start = m[0].second;

            int w0 = rend.dim.width, h0 = rend.dim.height;
            bool isPortrait = h0 > w0;
            int g = std::gcd(w0, h0);
            int ratioW = w0 / g, ratioH = h0 / g;

            std::vector<int> quals;
            quals.reserve(rend.sources.size());
            for (auto &r : rend.sources) {
                quals.push_back(int(r.scale));
            }
            std::sort(quals.begin(), quals.end());
            quals.erase(std::unique(quals.begin(), quals.end()), quals.end());

            // Turn that into a “360, 480, 720” string
            std::ostringstream optList;
            for (size_t i = 0; i < quals.size(); ++i) {
                optList << quals[i];
                if (i + 1 < quals.size()) optList << ", ";
            }

            {
                json::object cfg;
                cfg["selector"] = format("#videoplayer_{}", + video_num);

                json::object opts;
                opts["ratio"] = format("{}:{}", ratioW, ratioH);

                if (!isPortrait) {
                    opts["ratio"] = format("{}:{}", ratioW, ratioH);
                }

                json::object quality;
                auto defult_q = ContentManager::GetOptions().options.get("plyr.default", 0);
                if (ranges::any_of(quals, [defult_q](int q) { return q == defult_q; })) {
                    quality["default"] = defult_q;
                }

                json::array qarr;
                for (int h : quals)
                    qarr.push_back(h);
                quality["options"] = std::move(qarr);

                opts["quality"] = std::move(quality);
                cfg["options"] = std::move(opts);
                cfg["portrait"] = isPortrait;     // <-- add this flag

                // 4) Append to the array
                video_configs_.push_back(std::move(cfg));
            }
            ++video_num;
        }

        // 2) apply replacements *backwards* so offsets remain valid
        for (auto it = ops.rbegin(); it != ops.rend(); ++it) {
            std::size_t pos, len;
            std::string repl;
            std::tie(pos, len, repl) = *it;
            content.replace(pos, len, repl);
        }

        if (!ops.empty()) {
            using_video_ = true; // mark that this page contains a video
        }

        co_return;
    }

    boost::asio::awaitable<void>
    handleResponsiveImage(std::string& content, RenderCtx& ctx)
    {
        static const boost::regex img_pat{
            R"(!\[(.*?)\]\(([^;\)]+)(?:;(\d+px|\d+%|banner))?\))",
            boost::regex::normal | boost::regex::icase
        };

        boost::smatch m;
        size_t offset = 0;
        while (boost::regex_search(content.cbegin()+offset, content.cend(), m, img_pat)) {
            size_t start  = std::distance(content.cbegin(), m[0].first);
            size_t length = std::distance(m[0].first, m[0].second);

            std::string alt   = m[1];
            std::string src   = m[2];
            std::string size  = m[3];
            std::string name  = filesystem::path(src).stem().string();

            if (!src.ends_with(".jpg") && !src.ends_with(".jpeg")) {
                offset += length;
                continue;
            }

            filesystem::path image_path = ContentManager::GetOptions().source_path;
            image_path /= src;
            const auto images = GetImageMgr().Prepare(image_path);

            // decide attributes
            std::string sizes_attr;   // empty => omit sizes=
            std::string extra_style;  // e.g. " width:80%;"
            if (size == "banner") {
                sizes_attr = "100vw";
            } else if (!size.empty() && size.back() == '%') {
                // container-relative width; DO NOT set sizes=
                const int pct = std::clamp(std::stoi(size), 1, 100);
                extra_style = " width:" + std::to_string(pct) + "%;";
                sizes_attr.clear(); // omit sizes entirely for %
            } else if (!size.empty() && size.find("px") != std::string::npos) {
                sizes_attr = size;                 // ok for sizes=
                extra_style = " width:" + size + ";"; // also enforce visible width
            } else {
                // no ;size → let markdown handle (your chosen behavior)
                offset += length;
                continue;
            }

            // build HTML
            std::string html;
            if (size == "banner") {
                // <picture> art-directed banner
                html = "<picture>\n";
                const ImageMgr::ImageInfo* fallback = {};
                for (const auto& img : images) {
                    if (!fallback || (fallback->size.width < img.size.width && img.size.width <= 380))
                        fallback = &img;

                    html += "  <source media=\"(min-width: "
                            + std::to_string(img.size.width) + "px)\" srcset=\""
                            + ctx.getRelativePrefix() + img.relative_path + "\">\n";
                }
                if (fallback) {
                    html += "  <img src=\"" + ctx.getRelativePrefix() + fallback->relative_path
                            + "\" alt=\"" + alt + "\" loading=\"lazy\""
                            + " style=\"width:100%; height:auto; display:block;\">\n"
                            + "</picture>";
                }
            } else {
                // inline/content image with srcset
                std::string srcset;
                for (const auto& img : images) {
                    if (!srcset.empty()) srcset += ", ";
                    srcset += ctx.getRelativePrefix() + img.relative_path
                              + " " + std::to_string(img.size.width) + "w";
                }

                html = "<img src=\"" + ctx.getRelativePrefix()
                       + "/images/_scale_360/" + name + ".jpg\" "
                       + "srcset=\"" + srcset + "\" ";

                if (!sizes_attr.empty())
                    html += "sizes=\"" + sizes_attr + "\" ";

                html += "alt=\"" + alt + "\" loading=\"lazy\" "
                        // NOTE: omit sizes for %; CSS width enforces container-relative rendering
                        + "style=\"max-width:100%; height:auto; display:block;" + extra_style + "\">";
            }

            content.replace(start, length, html);
            offset = start + html.size();
        }

        co_return;
    }


    ImageMgr& GetImageMgr() {
        static const ImageMgr::widths_t scales{128, 248, 360, 480, 720, 1080, 1440, 2160};
        if (!image_mgr_) {
            image_mgr_ = ImageMgr::Create(scales, 80);
        }
        assert(image_mgr_);
        return *image_mgr_;
    }

    std::string getVideOptions() const override {
        auto opts = json::serialize(video_configs_);
        return opts;
    }

    // size_t numVideos() const noexcept override {
    //     return video_opts_.size();
    // }

    // std::string getVideOptions(size_t id) const override {
    //     if (id >= video_opts_.size()) {
    //         return {};
    //     }
    //     return video_opts_[id];
    // }

    const std::filesystem::path path_;
    const std::string content_;
    std::unique_ptr<ImageMgr> image_mgr_;
    bool using_video_ {false}; // true if the page contains a video
    json::array video_configs_;
};

page_t Page::Create(const std::filesystem::path& path) {
    return make_shared<PageImpl>(path);
}

page_t Page::Create(const string& content) {
    return make_shared<PageImpl>(content);
}

}

