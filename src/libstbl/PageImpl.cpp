
#include <string.h>
#include <fstream>
#include <regex>
#include <sstream>

#include <boost/algorithm/string/replace.hpp>

#include "stbl/stbl.h"
#include "stbl/Page.h"
#include "stbl/logging.h"
#include "stbl/utility.h"
#include "stbl/ContentManager.h"
#include "markdown.h"

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

        // Quick hack to extract code blocks and convert them to a <pre> block
        size_t pos = 0;
        while((pos != content.npos) && (pos < content.size())) {
            pos = content.find("```", pos);
            if ((pos != content.npos) && (content.size() >= (pos + 7))) {
                if ((pos > 0) && content.at(pos -1) != '\n') {
                    // start-tag is only valid at start of line
                    pos = content.find('\n', pos);
                    continue;
                }
                const auto spos = content.find('\n', pos);
                const auto epos = content.find("\n```\n", pos + 4);
                if (epos != content.npos) {
                    // pos = start ```
                    // spos = end of start-line
                    // epos is start of end ```

                    string code = content.substr(spos, epos - spos);
                    boost::replace_all(code, "<", "&lt;");
                    boost::replace_all(code, ">", "&gt;");

                    string code_block = "\n"s
                        + R"(<pre class="code">)"s
                        + code
                        + "\n</pre>\n"s;

                    content.replace(pos, (epos + 4) - pos, code_block);
                }
            } else {
                break;
            }
        }

        // Quick hack to handle images in series.
        static const std::regex images{R"(.*(!\[.+\])\((images\/.+)\))"};
        content = std::regex_replace(content, images, "$1("s + ctx.getRelativePrefix() + "$2)");

        istringstream stream_content(content);
        markdown::Document doc(stream_content);

        stringstream out_stream;
        doc.write(out_stream);

        content.assign(istreambuf_iterator<char>(out_stream),
                        istreambuf_iterator<char>());

        // We need to fix some potential problems in code-blocks, where the
        // the markdown process may escape &lt; and &gt;
        // TODO: Fix it in the markdown processor, as this hack may have side-effects
        boost::replace_all(content, "&amp;lt;", "&lt;");
        boost::replace_all(content, "&amp;gt;", "&gt;");

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

