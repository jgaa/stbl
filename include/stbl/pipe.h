#pragma once

#include <string>
#include <vector>

#include <boost/asio.hpp>
#include <boost/asio/redirect_error.hpp>
#include <boost/asio/experimental/awaitable_operators.hpp>
#include <boost/process/v2.hpp>
#include <boost/process.hpp>
#include "stbl/logging.h"

namespace stbl {

namespace detail {
std::string unfold(const auto& args) {
    std::ostringstream out;
    for (const auto& arg : args) {
        out << " " << arg;
    }
    return out.str();
};
}

// Based on https://stackoverflow.com/questions/79220553/boost-process-v2-how-to-asynchronously-read-output-and-also-check-for-terminati
template <typename T, typename L = std::vector<std::string>>
boost::asio::awaitable<std::string> popen(T& exexutor, std::string cmd, std::string input = {}, L args = {}) {
    namespace asio = boost::asio;
    namespace bp = boost::process::v2;

    // Without wrapping it in a strand, forcing the execution to be single-threaded,
    // I got lots of exception due to epoll() errors on Ubuntu.
    co_return co_await asio::co_spawn(exexutor, [&]() mutable -> asio::awaitable<std::string> {
        auto ex = co_await asio::this_coro::executor;
        LOG_DEBUG << "Running command: " << cmd << detail::unfold(args);

        asio::readable_pipe pout(ex), perr(ex);
        asio::writable_pipe pin(ex);
        bp::process         child{ex, boost::process::search_path(cmd), args, bp::process_stdio{.in = pin, .out = pout, .err = perr}};

        std::string output;

        auto read_loop = [&output, &cmd, &args](asio::readable_pipe& p) -> asio::awaitable<void> {
            for (std::array<char, 1024> buf;;) {
                auto [ec, n] = co_await p.async_read_some(asio::buffer(buf), asio::as_tuple(asio::deferred));
                if (n) {
                    output.append(buf.data(), n);
                }

                if (ec) {
                    if (ec == boost::asio::error::eof) {
                        break;
                    }
                    LOG_ERROR << "Read error while executing command: "
                              << cmd << detail::unfold(args) << ". Error: " << ec.message();
                    break; // or co_return;
                }
            }
        };

        // write-loop that writes the input to the child's stdin
        auto write_loop = [&input, &pin](asio::writable_pipe& p) -> asio::awaitable<void> {
            //co_await asio::async_write(pin, asio::buffer(input), asio::use_awaitable);
            auto written = 0u;
            do {
                written += co_await p.async_write_some(asio::buffer(input), asio::use_awaitable);
            } while (written < input.size());
            pin.close();
        };

        using namespace asio::experimental::awaitable_operators;
        int exit_code = co_await (                //
            write_loop(pin) &&                    //
            read_loop(pout) &&                    //
            read_loop(perr) &&                    //
            child.async_wait(asio::use_awaitable) //
            );

        LOG_TRACE << cmd << detail::unfold(args) << ": returned exit code " << exit_code;
        co_return output;
    });
}

// Not working. Seems like segments of output is missing.
// template <typename T, typename L = std::vector<std::string>>
// boost::asio::awaitable<std::string> popen(T& ex, std::string cmd, std::string input = {}, L args = {}) {
//     namespace bp = boost::process::v2;
//     namespace ba = boost::asio;

//     //auto ex = co_await ba::this_coro::executor;

//     LOG_DEBUG << "Queuing command: " << cmd << detail::unfold(args);

//     co_return co_await ba::co_spawn(ex, [&]() mutable -> ba::awaitable<std::string> {
//         LOG_DEBUG << "Running command: " << cmd << detail::unfold(args);
//         try {
//             bp::popen proc(ex, boost::process::search_path(cmd), args);
//             auto future = proc.async_wait(ba::use_future);
//             co_await ba::async_write(proc, ba::buffer(input), ba::use_awaitable);
//             proc.get_stdin().close();

//             std::ostringstream out;
//             std::array<char, 1024> buffer;
//             std::size_t bytes_read;
//             boost::system::error_code ec;
//             while (true) {
//                 try {
//                     bytes_read = co_await ba::async_read(proc, ba::buffer(buffer),ba::use_awaitable);
//                     out.write(buffer.data(), bytes_read);
//                 } catch (const boost::system::system_error& e) {
//                     const auto ec = e.code();
//                     if (ec) {
//                         if (ec == boost::asio::error::eof) {
//                             // End of file reached, exit the loop gracefully
//                             break;
//                         } else {
//                             LOG_ERROR << "Read error while executing command: " << cmd
//                                       << detail::unfold(args)
//                                       << ". Error: " << ec.message();

//                             throw;
//                         }
//                     }
//                 }
//             }
//             const auto rval = future.get();
//             if (rval) {
//                 LOG_WARN << "Command: '" << cmd
//                          << detail::unfold(args)
//                          << "' returned : " << rval;
//             }

//             co_return out.str();
//         } catch (const std::exception& e) {
//             LOG_ERROR << "Failed to run command: " << cmd
//                       << detail::unfold(args)
//                       << ". Error: " << e.what();
//             throw;
//         }
//         co_return std::string{};
//     });
// }

template <typename L = std::vector<std::string>>
boost::asio::awaitable<bool> run(std::string cmd, L args = {}) {
    namespace bp2 = boost::process::v2;
    namespace bp = boost::process;
    namespace ba = boost::asio;

    auto executor = co_await boost::asio::this_coro::executor;

    LOG_DEBUG << "Running command: " << cmd << detail::unfold(args);

    try {
        //co_await bp::system(executor, boost::process::search_path(cmd), args);
        bp2::process proc(executor, boost::process::search_path(cmd), args);
        const auto rval = co_await proc.async_wait(ba::use_awaitable);
        if (rval) {
            LOG_WARN << "Command: '" << cmd
                     << detail::unfold(args)
                     << "' returned : " << rval;
        }
        co_return rval == 0;
    } catch (const std::exception& e) {
        LOG_ERROR << "Failed to run command: " << cmd
                  << detail::unfold(args)
                  << ". Error: " << e.what();
    }
    co_return false;
}

} // namespace stbl
