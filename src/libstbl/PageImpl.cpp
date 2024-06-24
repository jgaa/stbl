
#include <string.h>
#include <fstream>
#include <regex>
#include <sstream>

#include <boost/algorithm/string/replace.hpp>

#include "cmark-gfm.h"

#include "stbl/stbl.h"
#include "stbl/Page.h"
#include "stbl/logging.h"
#include "stbl/utility.h"
#include "stbl/ContentManager.h"

using namespace std;

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

        // Quick hack to handle images in series.
        static const std::regex images{R"(.*(!\[.+\])\((images\/.+)\))"};
        content = std::regex_replace(content, images, "$1("s + ctx.getRelativePrefix() + "$2)");

        // Process markdown
        if (char * output{cmark_markdown_to_html(content.c_str(), content.size(),
            CMARK_OPT_DEFAULT | CMARK_OPT_VALIDATE_UTF8)}) {
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

