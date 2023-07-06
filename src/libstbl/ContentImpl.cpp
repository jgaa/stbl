
#include <filesystem>
#include <deque>
#include "stbl/stbl.h"
#include "stbl/Content.h"
#include "stbl/logging.h"
#include "stbl/Scanner.h"


using namespace std;

namespace stbl {


class ContentImpl : public Content
{
public:
    ContentImpl(const std::filesystem::path& path)
    : path_{path}
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

    void UpdateSourceHeaders(Scanner& scanner,
                             const Node::Metadata& meta) override {

        if (!meta.have_uuid || !meta.have_published) {
            scanner.UpdateRequiredHeaders(path_.string(), meta);
        }
    }

private:
    const std::filesystem::path path_;
    pages_t pages_;
};

content_t Content::Create(const std::filesystem::path& path) {
    return make_shared<ContentImpl>(path);
}

}

