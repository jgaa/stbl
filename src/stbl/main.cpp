#include <iostream>
#include <cstdlib>

#include <boost/program_options.hpp>
#include <boost/optional.hpp>
#include <boost/log/core.hpp>
#include <boost/log/trivial.hpp>
#include <boost/log/expressions.hpp>
#include <boost/filesystem.hpp>

#include "stbl/Options.h"
#include "stbl/logging.h"
#include "stbl/ContentManager.h"
#include "stbl/utility.h"

using namespace std;
namespace po = boost::program_options;
using namespace stbl;
using namespace std::string_literals;


void setup_logging(po::variables_map vm)
{
    namespace logging = boost::log;

    const static map<string, logging::trivial::severity_level> mapping = {
        {"error", logging::trivial::error},
        {"warning", logging::trivial::warning},
        {"warn", logging::trivial::warning},
        {"info", logging::trivial::info},
        {"debug", logging::trivial::debug},
        {"trace", logging::trivial::trace}};

    auto level = logging::trivial::info;
    if (vm.count("console-log")) {
        auto cmd_line_level = mapping.find(vm["console-log"].as<string>());
        if (cmd_line_level == mapping.end()) {
            cerr << "*** Log level '" << vm["console-log"].as<string>()
                << "' is undefined." << endl;
        }

        level = cmd_line_level->second;
    }

    logging::core::get()->set_filter
    (
        logging::trivial::severity >= level
    );
};


bool parse_command_line(int argc, char * argv[], Options &options)
{
    po::options_description general("General Options");

    general.add_options()
        ("help,h", "Print help and exit")
        ("console-log,C", po::value<string>(),
            "Log-level for the console-log")
        ("keep-tmp-dir,T", "Keep the temporary directory.")
        ("open-in-firefox,f", "Open the generated site in firefox.")
        ("publish,p", "Publish the site (deploy on a web-site).")
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
            << "Log-levels are:" << endl
            << "   error warning info debug trace " << endl;
        return false;
    }

    setup_logging(vm);

    if (vm.count("source-dir")) {
        options.source_path = vm["source-dir"].as<string>();
    } else {
        options.source_path = boost::filesystem::current_path().string();
    }

    if (vm.count("destination-dir")) {
        options.destination_path = vm["destination-dir"].as<string>();
    } else {
        const char *home = getenv("HOME");
        if (home == NULL) {
            cerr << "No destination specified, and no HOME environment variable set.";
            return false;
        }
        boost::filesystem::path dst_path = home;
        dst_path /= ".stbl-site";
        options.destination_path = dst_path.string();
    }

    if (vm.count("keep-tmp-dir")) {
        options.keep_tmp_dir = true;
    }

    if (vm.count("open-in-firefox")) {
        options.open_in_browser = "firefox";
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

    boost::filesystem::path opts = options.source_path;
    opts /= "stbl.conf";
    options.options = LoadProperties(opts);

    return true;
}

int main(int argc, char * argv[])
{
    Options options;

    if (!parse_command_line(argc, argv, options)) {
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
        boost::filesystem::path dst_path = options.publish
            ? options.options.get<string>("url")
            : options.destination_path;
        dst_path /= "index.html";
        string cmd = options.open_in_browser + " \""s + dst_path.string() + "\""s;
        system(cmd.c_str());
    }

    return 0;
}

