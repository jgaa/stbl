#pragma once

#include <memory>

#include "stbl/Node.h"
#include "stbl/Article.h"
#include "stbl/Content.h"
#include "stbl/Page.h"
#include "stbl/Series.h"
#include "stbl/Options.h"
#include "stbl/logging.h"


namespace stbl {

class ContentManager
{
protected:
    ContentManager() = default;

public:
    virtual ~ContentManager() = default;


    /*! Generate the site based on the managers options */
    virtual void ProcessSite() = 0;

public:
    /*! Factory */
    static std::shared_ptr<ContentManager> Create(const Options& options);
};

}
