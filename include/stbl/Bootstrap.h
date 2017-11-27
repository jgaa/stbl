#pragma once

#include <memory>

namespace stbl {

class Options;

/*! Bootstrap code for new sites
*/
class Bootstrap
{
public:
    virtual void CreateEmptySite(bool all) = 0;
    virtual void CreateNewExampleSite(bool all) = 0;

    static std::unique_ptr<Bootstrap> Create(Options& options);
};

}

