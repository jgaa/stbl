#include <iostream>
#include <cstdlib>
#include <filesystem>
#include <fstream>
#include <optional>

#include <boost/program_options.hpp>
#include <boost/process/v1/spawn.hpp>
#include <boost/process/v1/search_path.hpp>

#include "stbl/Options.h"
#include "stbl/logging.h"
#include "stbl/ContentManager.h"
#include "stbl/utility.h"
#include "stbl/stbl_config.h"
#include "stbl/Bootstrap.h"


using namespace std;
namespace po = boost::program_options;
using namespace stbl;
using namespace std::string_literals;

struct LogOptions {
    string console_log_level = "info";
    string log_level = "info";
    string log_file;
    bool truncate_log_file = false;
};


void setup_logging(const LogOptions& lo)
{
    const static map<string_view, logfault::LogLevel> mapping = {
        {"error", logfault::LogLevel::ERROR},
        {"warning", logfault::LogLevel::WARN},
        {"info", logfault::LogLevel::INFO},
        {"debug", logfault::LogLevel::DEBUGGING},
        {"trace", logfault::LogLevel::TRACE}};

    auto to_level = [](string_view name) -> optional<logfault::LogLevel> {
        if (name.empty() || name == "off" || name == "false") {
            return {};
        }

        if (auto it = mapping.find(name); it != mapping.end()) {
            return it->second;
        }

        return {};
    };


    if (auto level = to_level(lo.console_log_level)) {
        logfault::LogManager::Instance().AddHandler(
            make_unique<logfault::StreamHandler>(clog, *level));
    }

    if (!lo.log_file.empty()) {
        if (auto level = to_level(lo.log_level)) {
            logfault::LogManager::Instance().AddHandler(
                make_unique<logfault::StreamHandler>(lo.log_file, *level, lo.truncate_log_file));
        }
    }
};


bool parse_command_line(int argc, char * argv[], Options &options)
{
    LogOptions log_options;
    po::options_description general("General Options");

    general.add_options()
        ("help,h", "Print help and exit")
        ("log-to-console,C",
         po::value(&log_options.console_log_level)->default_value(log_options.console_log_level),
         "log level to the console; one of 'error', 'warn', 'info', 'debug', 'trace'. Empty string to disable.")
        ("log-level",
         po::value<string>(&log_options.log_level)->default_value(log_options.log_level),
         "log level; one of 'error', 'warn', 'info', 'debug', 'trace'.")
        ("log-file",
         po::value<string>(&log_options.log_file),
         "Log file to write a log to. Default is to use only the console.")
        ("truncate-log-file",
         po::bool_switch(&log_options.truncate_log_file),
         "Truncate the logfile if it already exists.")
        ("keep-tmp-dir,T", "Keep the temporary directory.")
        ("open-in-firefox,f", "Open the generated site in firefox.")
        ("open-in-browser,b", "Open the generated site in the defaut browser.")
        ("publish,p", "Publish the site (deploy on a web-site).")
        ("no-update-headers", "Do not update the source article headers.")
        ("automatic-update,u", po::value(&options.automatic_update)->default_value(options.automatic_update),
            "Automatically set the updated attribute if the file-time is newer than the publish-time")
        ("preview", "Do not update the source article headers. Generate all articles.")
        ("version,v", "Show version and exit.")
        ("init", "Initialize a new blog directory structure at the destination.")
        ("init-all", "Initialize a new blog directory structure at the destination, including templates and embedded files.")
        ("init-example", "Initialize a new example blog directory structure at the destination.")
        ("threads", po::value(&options.threads)->default_value(options.threads),
          "Number of threads to use for processing. Default is -1 (auto).)")
        ;

    po::options_description locations("Locations");
    locations.add_options()
        ("source-dir,s",  po::value<string>(),
            "Directory for the sites content. Defaults to the current directory")
        ("destination-dir,d",  po::value<string>(),
            "Where to put the generated site (locally). Defaults to $HOME/.stbl-site")
        ("content-layout,L", po::value<string>()->default_value("simple"),
            "How to organize the site. 'simple' or 'recursive'.")
        ("publish-to,P",  po::value<string>(),
            "Publish the site to <location>. Implicitly enables --publish.")
        ;

    po::options_description cmdline_options;
    cmdline_options.add(general).add(locations);

    po::variables_map vm;
    po::store(po::parse_command_line(argc, argv, cmdline_options), vm);
    po::notify(vm);

    if (vm.count("help")) {
        cout << "stbl [options]" << endl
            << cmdline_options << endl
             << "log levels are:" << endl
             << " error warning info debug trace " << endl;
        return false;
    }

    if (vm.count("version")) {
        cout << "stbl " << STBL_VERSION << endl;
        return false;
    }

    setup_logging(log_options);

    if (vm.count("source-dir")) {
        options.source_path = vm["source-dir"].as<string>();
    } else {
        options.source_path = std::filesystem::current_path().string();
    }

    if (vm.count("destination-dir")) {
        options.destination_path = vm["destination-dir"].as<string>();
    } else {
        const char *home = getenv("HOME");
        if (home == NULL) {
            cerr << "No destination specified, and no HOME environment variable set.";
            return false;
        }
        std::filesystem::path dst_path = home;
        dst_path /= ".stbl-site";
        options.destination_path = dst_path.string();
    }

    if (vm.count("keep-tmp-dir")) {
        options.keep_tmp_dir = true;
    }

    if (vm.count("open-in-browser")) {
        if (filesystem::is_regular_file("/usr/bin/sensible-browser")) {
            options.open_in_browser = "sensible-browser";
        } else {
            options.open_in_browser = "xdg-open";
        }
    }

    if (vm.count("open-in-firefox")) {
        options.open_in_browser = "firefox";
    }

    if (vm.count("no-update-headers")) {
        options.update_source_headers = false;
    }

    if (vm.count("preview")) {
        options.update_source_headers = false;
        options.preview_mode = true;
    }

    if (vm.count("publish")) {
        options.publish = true;
    }

    if (vm.count("publish-to")) {
        options.publish_destination = vm["publish-to"].as<string>();
        options.publish = true;
    }

    if (vm.count("content-layout")) {
        const auto val = vm["content-layout"].as<string>();
        if (val == "simple") {
            options.path_layout = Options::PathLayout::SIMPLE;
        } else if (val == "recursive") {
            options.path_layout = Options::PathLayout::RECURSIVE;
        } else {
            cerr << "Unknown content-layout" << val << endl;
            return false;
        }

    }

    if (vm.count("init")) {
        auto bootstrap = Bootstrap::Create(options);
        bootstrap->CreateEmptySite(false);
        return false;
    }

    if (vm.count("init-all")) {
        auto bootstrap = Bootstrap::Create(options);
        bootstrap->CreateEmptySite(true);
        return false;
    }

    if (vm.count("init-example")) {
        auto bootstrap = Bootstrap::Create(options);
        bootstrap->CreateNewExampleSite(true);
        return false;
    }

    std::filesystem::path opts = options.source_path;
    opts /= "stbl.conf";
    options.options = LoadProperties(opts);

    return true;
}

int main(int argc, char * argv[])
{
    Options options;

    try {
        if (!parse_command_line(argc, argv, options)) {
            return -1;
        }
    } catch (std::exception& ex) {
        cerr << "*** Failed to parse command line: " << ex.what() << endl;
        return -1;
    }

    LOG_INFO << "Ready to process '" << options.source_path
        << "' --> '" << options.destination_path << "'";

    try {
        auto manager = ContentManager::Create(options);
        manager->ProcessSite();
    } catch (std::exception& ex) {
        LOG_ERROR << "*** Failed to process site: " << ex.what();
        return -1;
    }

    if (!options.open_in_browser.empty()) {
        std::filesystem::path dst_path = options.publish
            ? options.options.get<string>("url")
            : options.destination_path;
        dst_path /= "index.html";
        LOG_DEBUG << "Executing: " << options.open_in_browser << ' ' << dst_path;
            try {
            boost::process::v1::spawn(
                boost::process::v1::search_path(options.open_in_browser),
                dst_path.c_str());
            LOG_DEBUG << "Done starting the browser";
        } catch (std::exception& ex) {
            LOG_ERROR << "Failed to start the browser: " << ex.what();
        }
    }

    LOG_INFO << "Done";

    return 0;
}

