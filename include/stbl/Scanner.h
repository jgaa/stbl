#pragma once

#include <memory>

#include "stbl/stbl.h"
#include "stbl/Options.h"


namespace stbl {

class Scanner
{
public:
    Scanner() = default;
    virtual ~Scanner() = default;

    virtual nodes_t Scan() = 0;

    static std::unique_ptr<Scanner> Create(const Options& options);
};

}
