#pragma once

#include "stbl/stbl.h"

namespace stbl {

class Content
{
public:
    Content() = default;
    virtual ~Content() = default;

    virtual void AddPage(page_t page) = 0;
    virtual pages_t GetPages() = 0;

    static content_t Create();
};

}
