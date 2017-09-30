
#include <deque>
#include "stbl/stbl.h"
#include "stbl/Content.h"

using namespace std;

namespace stbl {


class ContentImpl : public Content
{
public:
    ContentImpl()
    {
    }

    ~ContentImpl()  {
    }

    void AddPage(page_t page) override {
        pages_.push_back(move(page));
    }

    pages_t GetPages() override {
        return pages_;
    }

private:
    pages_t pages_;
};

content_t Content::Create() {
    return make_shared<ContentImpl>();
}

}

