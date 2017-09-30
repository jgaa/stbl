#pragma once

#include <ostream>


#include <boost/log/trivial.hpp>
#include <boost/log/core.hpp>
#include <boost/log/expressions.hpp>
#include <boost/log/attributes.hpp>
#include <boost/log/sources/basic_logger.hpp>
#include <boost/log/sources/severity_logger.hpp>
#include <boost/log/sources/record_ostream.hpp>
#include <boost/log/sinks/sync_frontend.hpp>
#include <boost/log/sinks/text_ostream_backend.hpp>
#include <boost/log/attributes/scoped_attribute.hpp>
#include <boost/log/utility/setup/common_attributes.hpp>

#define LOG_ERROR     BOOST_LOG_TRIVIAL(error)
#define LOG_WARN      BOOST_LOG_TRIVIAL(warning)
#define LOG_INFO      BOOST_LOG_TRIVIAL(info)
#define LOG_DEBUG     BOOST_LOG_TRIVIAL(debug)
#define LOG_TRACE     BOOST_LOG_TRIVIAL(trace)

#include "stbl/Node.h"

namespace stbl {
::std::ostream& operator << (::std::ostream& out, const ::stbl::Node::Type& value);
::std::ostream& operator << (::std::ostream& out, const ::stbl::Node& node);
}
