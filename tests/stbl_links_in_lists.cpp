
#include <boost/log/trivial.hpp>
#include <boost/algorithm/string.hpp>
#include "stbl/stbl.h"
#include "stbl_tests.h"
#include "stbl/Page.h"

using namespace std;
using namespace stbl;

const lest::test specification[] = {

STARTCASE(TestLinkInList) {

    const std::string source =
        "---\n"
        "title: Test\n"
        "---\n"
        "- [This](https://example.com) is a link to [github](https://gitub.com).";

    std::stringstream out;

    auto page = Page::Create(source);

    page->Render2Html(out);

    CHECK_EQUAL(boost::trim_right_copy(out.str()),
                R"(<p>- <a href="https://example.com">This</a> is a link to <a href="https://gitub.com">github</a>.</p>)");

} ENDCASE
}; //lest

int main( int argc, char * argv[] )
{
    namespace logging = boost::log;
    logging::core::get()->set_filter
    (
        logging::trivial::severity >= logging::trivial::info
    );
    return lest::run( specification, argc, argv );
}

