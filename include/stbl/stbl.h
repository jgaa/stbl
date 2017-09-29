#pragma once

#include <memory>
#include <vector>


namespace stbl {

class Node;
class Article;
class Page;

using nodes_t = std::vector<std::shared_ptr<Node>>;
using articles_t = std::vector<std::shared_ptr<Article>>;
using pages_t = std::vector<std::shared_ptr<Page>>;

}
