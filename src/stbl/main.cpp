#include <iostream>

#include <boost/program_options.hpp>
#include <boost/optional.hpp>
#include <boost/log/core.hpp>
#include <boost/log/trivial.hpp>
#include <boost/log/expressions.hpp>
#include <boost/filesystem.hpp>

#include "stbl/Options.h"
#include "stbl/logging.h"
#include "stbl/ContentManager.h"

using namespace std;
namespace po = boost::program_options;
using namespace stbl;


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
        ;

    po::options_description locations("Locations");
    locations.add_options()
        ("source-dir,s",  po::value<string>(),
            "Directory for the sites content. Defaults to the current directory")
        ("destination-dir,d",  po::value<string>(),
            "Where to put the generated site (locally). Defaults to $HOME/.stbl-site")
        ;

    po::options_description cmdline_options;
    cmdline_options.add(general).add(locations);

    po::variables_map vm;
    po::store(po::parse_command_line(argc, argv, cmdline_options), vm);
    po::notify(vm);

    if (vm.count("help")) {
        cout << cmdline_options << endl
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
        options.destination_path = "~/.stbl-site";
    }

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

    return 0;
}

