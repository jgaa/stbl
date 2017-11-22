#pragma once

#include "stbl/logging.h"
#include "lest/lest.hpp"

#define STARTCASE(name) { CASE(#name) { \
    LOG_DEBUG << "================================"; \
    LOG_INFO << "Test case: " << #name; \
    LOG_DEBUG << "================================";

#define ENDCASE \
    LOG_DEBUG << "============== ENDCASE ============="; \
}},

