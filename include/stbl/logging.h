#pragma once

#include <boost/log/trivial.hpp>

#define LOG_ERROR     BOOST_LOG_TRIVIAL(error)
#define LOG_WARN      BOOST_LOG_TRIVIAL(warning)
#define LOG_INFO      BOOST_LOG_TRIVIAL(info)
#define LOG_DEBUG     BOOST_LOG_TRIVIAL(debug)
#define LOG_TRACE     BOOST_LOG_TRIVIAL(trace)


