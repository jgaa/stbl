
#include <boost/lexical_cast.hpp>

#include "stbl/Options.h"
#include "stbl/ContentManager.h"
#include "stbl/Scanner.h"
#include "stbl/Node.h"
#include "stbl/Series.h"
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

        LOG_DEBUG << "Listing nodes after scan: ";
        for(const auto& n: nodes_) {
            LOG_DEBUG << "   " << *n;

            if (n->GetType() == Node::Type::SERIES) {
                const auto& series = dynamic_cast<const Series&>(*n);
                for(const auto& a : series.GetArticles()) {
                    LOG_DEBUG << "      ---> " << *a;
                }
            }
        }
    }

    void Prepare()
    {
        // Go over tags.
        //    - Create a list of all tags
        //    - Add reference to article
        //    - Sort the list

        // Go over subjects
        //    - Create a list of all subjects
        //    - Add reference to article
        //    - Sort the list

        // Decide the location and url for each article

        // Create a list of all valid root-level nodes and sort it
    }

    void MakeTempSite()
    {
        // Create the main page from template

        // Create an overview page with all published articles in a tree.

        // Create XSS feed pages.
        //    - One global
        //    - One for each subject

        // Render the series and articles

        // Copy artifacts, images and other files
    }

    void CommitToDestination()
    {
        // Make checksums for all the files in the tmp site.
        // Make checksums of the files in the destination site.
        // Copy all files that have changed.
    }

    void CleanUp()
    {
        // Remove the temp site
        // Remove any other temporary files
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

