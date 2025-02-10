
#include <fstream>
#include <streambuf>
#include <iomanip>
#include <ctime>
#include <iostream>
#include <codecvt>
#include <filesystem>
#include <string_view>
#include <thread>

#include <boost/property_tree/ptree.hpp>
#include <boost/property_tree/info_parser.hpp>
#include <boost/lexical_cast.hpp>
#include <boost/uuid/uuid.hpp>
#include <boost/uuid/uuid_io.hpp>
#include <boost/uuid/uuid_generators.hpp>
#include <boost/asio.hpp>

#include "stbl/utility.h"
#include "stbl/logging.h"

using namespace std;
using namespace std::string_literals;
//using boost::string_ref;
namespace pt = boost::property_tree;
namespace fs = std::filesystem;
namespace asio = boost::asio;

namespace stbl {

string Load(const fs::path& path) {

    if (!filesystem::is_regular_file(path)) {
        LOG_ERROR << "The file " << path << " need to exist!";
        throw runtime_error("I/O error - Missing required file.");
    }

    std::ifstream t(path.string());
    string str;

    t.seekg(0, std::ios::end);
    str.reserve(t.tellg());
    t.seekg(0, std::ios::beg);

    str.assign((std::istreambuf_iterator<char>(t)),
        std::istreambuf_iterator<char>());

    return str;
}

void Save(const fs::path& path,
          const string& data,
          bool createDirectoryIsMissing,
          bool binary) {

    LOG_TRACE << "Saving: " << path
        << (binary ? " [bin]" : " [text]");


    if (createDirectoryIsMissing) {
        CreateDirectoryForFile(path);
    }

    auto mode = ios_base::out | ios_base::trunc;
    if (binary) {
        mode |= ios_base::binary;
    }
    std::ofstream out(path.string(), mode);

    if (!out) {
        auto err = strerror(errno);
        LOG_ERROR << "IO error. Failed to open "
            << path << " for write: " << err;

        throw runtime_error("IO error");
    }

    out << data;
}

void CreateDirectoryForFile(const std::filesystem::path& path) {
    const auto directory = path.parent_path();
    if (!is_directory(directory)) {
        CreateDirectory(directory);
    }
}

void CreateDirectory(const std::filesystem::path& path) {
    if (!is_directory(path)) {
        LOG_DEBUG << "Creating directory: " << path;
        create_directories(path);
    }
}

boost::property_tree::ptree
LoadProperties(const fs::path& path) {
    if (!fs::is_regular_file(path)) {
        LOG_ERROR << "The file " << path << " need to exist!";
        throw runtime_error("I/O error - Missing required file.");
    }

    LOG_TRACE << "Loading properties" << path;
    pt::ptree tree;
    pt::read_info(path.string(), tree);
    return tree;
}

string ToString(const std::wstring& str) {
    wstring_convert<codecvt_utf8<wchar_t>> converter;
    return converter.to_bytes(str);
}

std::wstring ToWstring(const string& str) {
    wstring_convert<std::codecvt_utf8_utf16<wchar_t>> converter;
    return converter.from_bytes(str);
}

string ToStringAnsi(const time_t& when) {
    if (when) {
        if (const auto tm = std::localtime(&when)) {
            return boost::lexical_cast<string>(put_time(tm, "%F %R"));
        }
    }
    return {};
}

time_t Roundup(time_t when, const int roundup) {
    if (!when) {
        return {};
    }

    const bool add = (when % roundup) != 0;
    when /= roundup;
    when *= roundup;
    if (add) {
        when += roundup;
    }
    return when;
}

void CopyDirectory(const fs::path& src,
                   const fs::path& dst) {

    if (!is_directory(src)) {
        LOG_ERROR << "The dirrectory "
            << src << " need to exist in order to copy it!";
        throw runtime_error("I/O error - Missing required directory.");
    }

    if (!is_directory(dst)) {
        create_directories(dst);
    }

    for (const auto& de : fs::directory_iterator{src})
    {
        fs::path d = dst;
        d /= de.path().filename();
        LOG_TRACE << "Copying " << de.path() << " --> " << d;
        if (fs::is_regular_file(de.path())) {
            fs::copy_file(de.path(), d, fs::copy_options::overwrite_existing);
        } else if (is_symlink(de.path())) {
            fs::copy_symlink(de.path(), d);
        } else if (is_directory(de.path())) {
            CopyDirectory(de.path(), d);
        }  else {
            LOG_WARN << "Skipping " << de.path()
                << " from directory copy. I don't know what it is...";
        }
    }
}

void EatHeader(std::istream& in) {

    int separators = 0;

    if (!in) {
        throw runtime_error("Parse error: Empty file?");
    }

    if (in.peek() == 0xef) {
        in.get();
        if ((!in || in.get() != 0xbb) || (!in || in.get() != 0xbf)) {
            throw runtime_error("Parse error: Invalid file format (failed to parse BOM)");
        }
    }

    while(in) {
        if ((in && in.get() == '-')
            && (in && (in.get() == '-'))
            && (in && (in.get() == '-'))) {
            ++separators;
        }

        while(in && (in.get() != '\n'))
            ;

        if (separators == 2) {
            return;
        }
    }

    throw runtime_error("Parse error: Failed to locate header section.");
}

string CreateUuid() {
    boost::uuids::uuid uuid = boost::uuids::random_generator()();
    return boost::uuids::to_string(uuid);
}

std::filesystem::path MkTmpPath()
{
    auto path = std::filesystem::temp_directory_path();
    path /= CreateUuid();
    return path;
}


// string Pipe(const string& cmd,
//             const std::vector<string>& args,
//             const string& input)
// {
//     namespace bp = boost::process;
//     bp::ipstream pipe_out; // To read from stdout
//     bp::opstream pipe_in;  // To write to stdin
//     ostringstream output;

//     try {
//         bp::child c(boost::process::search_path(cmd), bp::args(args), bp::std_out > pipe_out, bp::std_in < pipe_in);

//         // Write buffer to the process's stdin
//         pipe_in.write(input.data(), input.size());
//         pipe_in.flush();
//         pipe_in.pipe().close(); // Close the pipe to indicate end of input

//         string line;
//         while (pipe_out.is_open() && !pipe_out.eof()) {
//             getline(pipe_out, line);
//             output << line << std::endl;
//         }
//     } catch (const std::exception& e) {
//         LOG_ERROR << "Failed to run command: " << cmd << " " << e.what();
//         throw;
//     }

//     return output.str();
// };

// asio::awaitable<string> Pipe(const string& cmd, const vector<string>& args, const string& input) {
//     //auto executor = co_await asio::this_coro::executor;
//     //auto& io_context = static_cast<asio::io_context&>(executor.context());  // ✅ Correct way to get io_context
//     //auto& io_context = asio::get_executor(executor).context();  // ✅ Correct way to

//     static boost::asio::io_context io_context;
//     static std::thread io_thread([&] {
//         io_context.run();
//     });

//     namespace bp = boost::process::v2;
//     bp::async_pipe pipe_out(io_context);
//     bp::async_pipe pipe_in(io_context);
//     ostringstream output;

//     // Lambda to unfold args
//     auto unfold = [](const string& acc, const string& arg) {
//         return acc + " " + arg;
//     };

//     accumulate(begin(args), end(args), string(), unfold);

//     LOG_DEBUG << "Running command: " << cmd << " " << accumulate(begin(args), end(args), string(), unfold);

//     try {
//         // Start the process asynchronously
//         bp::child c(
//             bp::search_path(cmd),
//             bp::args(args),
//             bp::std_out > pipe_out,
//             bp::std_in < pipe_in,
//             io_context
//             );

//         // Write input asynchronously
//         co_await asio::async_write(pipe_in, asio::buffer(input), asio::use_awaitable);
//         pipe_in.close();  // Close stdin after writing input

//         // Read output asynchronously
//         std::vector<char> buffer(4096);namespace bp = boost::process;
//         std::size_t bytes_read = co_await asio::async_read(pipe_out, asio::buffer(buffer), asio::use_awaitable);

//         output.write(buffer.data(), bytes_read);
//         pipe_out.close();

//         // Ensure child process completes
//         c.wait();
//     } catch (const std::exception& e) {
//         LOG_ERROR << "Failed to run command: " << cmd << " "
//              << accumulate(begin(args), end(args), string(), unfold)
//              << ". Error: " << e.what() << endl;
//         co_return "";
//     }

//     co_return output.str();
// }

} // ns
