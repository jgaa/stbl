
#include "stbl/Options.h"
#include "stbl/ContentManager.h"
#include "stbl/Scanner.h"
#include "stbl/logging.h"

using namespace std;

namespace stbl {

class ContentManagerImpl : public ContentManager
{
public:
    ContentManagerImpl(const Options& options)
    : options_{options}
    {
    }

    ~ContentManagerImpl() {
        CleanUp();
    }

    void ProcessSite() override
    {
        Scan();
        Prepare();
        MakeTempSite();
        CommitToDestination();
    }


protected:
    void Scan()
    {
        auto scanner = Scanner::Create(options_);
        nodes_= scanner->Scan();
    }

    void Prepare()
    {
    }

    void MakeTempSite()
    {
    }

    void CommitToDestination()
    {
    }

    void CleanUp()
    {
    }

protected:
    Options options_;
    nodes_t nodes_;
};

std::shared_ptr<ContentManager> ContentManager::Create(const Options& options)
{
    return make_shared<ContentManagerImpl>(options);
}

}

