
#include <boost/log/trivial.hpp>
#include "stbl/stbl.h"
#include "stbl_tests.h"
#include "stbl/Page.h"

using namespace std;
using namespace stbl;

const lest::test specification[] = {

STARTCASE(Test_WfdePath) {

    const std::string source =
        "---\n"
        "title: Test\n"
        "---\n"
        "- [This](https://example.com) is a link to [github](https://gitub.com).";

    std::stringstream out;

    auto page = Page::Create(source);

    page->Render2Html(out);

    cout << "Result: '" << out.str() << '\'' << endl;

} ENDCASE
}; //lest

int main( int argc, char * argv[] )
{
    namespace logging = boost::log;
    logging::core::get()->set_filter
    (
        logging::trivial::severity >= logging::trivial::trace
    );
    return lest::run( specification, argc, argv );
}

