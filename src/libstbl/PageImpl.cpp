
#include <string.h>
#include <fstream>

#include "stbl/stbl.h"
#include "stbl/Page.h"
#include "stbl/logging.h"
#include "markdown.h"

using namespace std;

namespace stbl {


class PageImpl : public Page
{
public:
    PageImpl(const boost::filesystem::path& path)
    : path_{path}
    {
    }

    ~PageImpl()  {
    }

    void Render2Html(std::ostream & out) override {

        // TODO: Generate from template so that we get a full page with navigation

        ifstream in(path_.string());
        if (!in) {
            auto err = strerror(errno);
            LOG_ERROR << "IO error. Failed to open "
                << path_ << ": " << err;

            throw runtime_error("IO error");
        }

        EatHeader(in);

        markdown::Document doc(in);
        doc.write(out);
    }

private:
    void EatHeader(std::istream& in) {

        int separators = 0;

        while(in) {
            if ((in && in.get() == '-')
                && (in && (in.get() == '-'))
                && (in && (in.get() == '-'))) {
                ++separators;
            }

            while(in && (in.get() != '\n'))
                ;

            if (separators == 2) {
                return;
            }
        }

        throw runtime_error("Parse error: Failed to locate header section.");
    }

    const boost::filesystem::path path_;
};

page_t Page::Create(const boost::filesystem::path& path) {
    return make_shared<PageImpl>(path);
}

}

