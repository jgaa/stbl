
#include <map>
#include <locale>
#include <codecvt>
#include <iomanip>
#include <ctime>


#include "stbl/Node.h"

namespace stbl {

::std::ostream& operator << (::std::ostream& out, const stbl::Node::Type& value) {
    const static std::vector<const char *> mapping = { "ARTICLE", "SERIES" };

    return out << mapping.at(static_cast<unsigned>(value));
}

::std::ostream& operator << (::std::ostream& out, const stbl::Node& node) {

    const auto meta = node.GetMetadata();
    std::string name;
    if (meta) {
        std::wstring_convert<::std::codecvt_utf8_utf16<wchar_t>> converter;
        name = converter.to_bytes(meta->title);
    }

    return out << '\"' << name << "\" (" << node.GetType() << ')';
}


}
