#include "stbl/logging.h"

using namespace std;
using boost::string_ref;

namespace stbl {

// boost::string_ref Sf(boost::string_ref::const_iterator start,
//                      boost::string_ref::const_iterator end,
//                      bool trim) {
//     boost::string_ref sf = {start, static_cast<std::size_t>(end - start)};
//     if (trim) {
//         while (!sf.empty()
//             && ((sf.front() == '\n')
//                 || (sf.front() == '\t')
//                 || (sf.front() == ' '))) {
//             sf = {sf.data() + 1, sf.size() -1};
//         }
//         while (!sf.empty()
//             && ((sf.back() == '\n')
//                 || (sf.back() == '\t')
//                 || (sf.back() == ' '))) {
//             sf = {sf.data(), sf.size() -1};
//         }
//     }
//     return sf;
// }

}
