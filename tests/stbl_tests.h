#pragma once

#include <cstdlib>
#include <boost/algorithm/string.hpp>
#include "stbl/logging.h"
#include "lest/lest.hpp"

namespace stbl {
namespace {

#define STARTCASE(name) { CASE(#name) { \
    LOG_DEBUG << "================================"; \
    LOG_INFO << "Test case: " << #name; \
    LOG_DEBUG << "================================";

#define ENDCASE \
    LOG_DEBUG << "============== ENDCASE ============="; \
}},

template<typename T1, typename T2>
bool compare(const T1& left, const T2& right) {
    const auto state = (left == right);
    if (!state) {
        std::cerr << ">>>> '" << left << "' is not equal to '" << right << "'" << std::endl;
    }
    return state;
}
} // anonymous namespace

template<typename T>
bool CHECK_CLOSE(const T expect, const T value, const T slack) {
    return (value >= (expect - slack))
        && (value <= (expect + slack));
}

} // namespace


#define CHECK_EQUAL(a,b) EXPECT(compare(a,b))
#define CHECK_EQUAL_ENUM(a,b) EXPECT(compare(static_cast<int>(a), static_cast<int>(b)))
#define TEST(name) CASE(#name)

