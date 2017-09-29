#pragma once

#include "stbl/stbl.h"

namespace stbl {

class Content
{
public:
    Content() = default;
    virtual ~Content() = default;

    virtual pages_t GetPages();
};

}
