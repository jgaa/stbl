#pragma once

#include <ostream>

#include "stbl/stbl.h"

namespace stbl {

class Page
{
public:
    Page() = default;
    virtual ~Page() = default;

    void Render2Html(std::ostream& out);

    std::wstring content;
};

}

