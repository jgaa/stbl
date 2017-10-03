#pragma once

#include <memory>
#include <vector>

namespace stbl {

class Node;
class Article;
class Series;
class Page;
class Content;

using node_t = std::shared_ptr<Node>;
using nodes_t = std::vector<node_t>;

using article_t = std::shared_ptr<Article>;
using articles_t = std::vector<article_t>;

using serie_t = std::shared_ptr<Series>;

using page_t = std::shared_ptr<Page>;
using pages_t = std::vector<page_t>;

using content_t = std::shared_ptr<Content>;

#ifndef PROGRAM_NAME
#   define PROGRAM_NAME "stbl"
#endif

#ifndef PROGRAM_VERSION
#   define PROGRAM_VERSION "0.01-devel"
#endif

}
