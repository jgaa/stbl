

#include <deque>
#include "stbl/stbl.h"
#include "stbl/Page.h"

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

    }

private:
    const boost::filesystem::path path_;
};

page_t Page::Create(const boost::filesystem::path& path) {
    return make_shared<PageImpl>(path);
}

}

