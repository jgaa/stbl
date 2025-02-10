
#include <map>
#include <locale>
#include <codecvt>
#include <iomanip>
#include <ctime>


#include "stbl/Node.h"
#include "stbl/logging.h"

namespace stbl {

::std::ostream& operator << (::std::ostream& out, const stbl::Node::Type& value) {
    const static std::vector<const char *> mapping = { "ARTICLE", "SERIES" };

    return out << mapping.at(static_cast<unsigned>(value));
}

::std::ostream& operator << (::std::ostream& out, const stbl::Node& node) {

    const auto meta = node.GetMetadata();
    std::string name;
    std::string uuid;
    if (meta) {
        name = toString(meta->title);
        uuid = meta->uuid;
    }

    return out << uuid << " \"" << name << "\" (" << node.GetType() << ')';
}


}
