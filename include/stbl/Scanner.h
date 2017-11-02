#pragma once

#include <memory>

#include "stbl/stbl.h"
#include "stbl/Node.h"
#include "stbl/Options.h"


namespace stbl {

class Scanner
{
public:
    Scanner() = default;
    virtual ~Scanner() = default;

    virtual nodes_t Scan() = 0;

    // Called when the article is rendered to make sure the published
    // and uuid headers are set and saved.
    // Both are required to support rss feeds.
    virtual void UpdateRequiredHeaders(const std::string& article,
                                       const Node::Metadata& meta) = 0;

    static std::unique_ptr<Scanner> Create(const Options& options);

};

}
