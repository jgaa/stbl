
#include "stbl/stbl.h"
#include "stbl/utility.h"
#include "stbl/Bootstrap.h"
#include "stbl/Options.h"
#include "stbl/logging.h"

#include "templates_res.h"
#include "artifacts_res.h"
#include "config_res.h"
#include "articles_res.h"



using namespace std;
using namespace std::string_literals;

namespace stbl {


class BootstrapImpl : public Bootstrap
{
public:
    BootstrapImpl(const Options& options)
    : options_{options}
    {
    }

    void CreateEmptySite(bool all) override {
        const filesystem::path root = options_.source_path;

        LOG_INFO << "Initializing new site: " << root;

        // Create config
        auto conf_path = root;
        conf_path /= "stbl.conf";
        Save(conf_path, Get(embedded_config_, "stbl.conf"), true);

        // Create directories
        for(const auto& name : initializer_list<string> {
            "articles", "images", "files", "artifacts", "templates"} ) {
            filesystem::path p = root;
            p /= name;
            CreateDirectory(p);
        }

        // Create artifacts
        filesystem::path artifacts = root;
        artifacts /= "artifacts";
        SaveList(embedded_artifacts_, artifacts);

        if (all) {
            filesystem::path templates = root;
            templates /= "templates";
            SaveList(embedded_templates_, templates);
        }
    }

    void CreateNewExampleSite(bool all) override {
        const filesystem::path root = options_.source_path;
        CreateEmptySite(all);
        filesystem::path articles = root;
        articles /= "articles";
        SaveList(embedded_articles_, articles);
    }

private:
    template <typename T>
    std::string Get(const T& map, const std::string& name) {
        auto it = map.find(name);
        if (it == map.end()) {
            throw runtime_error("Missing embedded resource: "s + name);
        }

        return string(reinterpret_cast<const char *>(it->second.first), it->second.second);
    }

    template <typename T>
    void SaveList(const T& list, const filesystem::path& dir) {
        for(const auto& it: list) {
            filesystem::path p = dir;
            p /= it.first;

            auto ext = p.extension();

            auto is_bin = (ext == ".jpg");

            if (is_bin) {
                int i = 1;
            }

            string data(reinterpret_cast<const char *>(it.second.first),
                        it.second.second);

            Save(p, data, true, is_bin);
        }
    }

    const Options& options_;
};

std::unique_ptr<Bootstrap> Bootstrap::Create(Options& options) {
    return make_unique<BootstrapImpl>(options);
}

}


