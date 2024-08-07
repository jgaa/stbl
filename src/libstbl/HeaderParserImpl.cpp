
#include <iostream>
#include <string>
#include <locale>
#include <codecvt>
#include <sstream>
#include <iomanip>
#include <ctime>

#include <boost/spirit/include/classic.hpp>
#include <boost/config/warning_disable.hpp>
#include <boost/spirit/include/qi.hpp>
#include <boost/spirit/include/phoenix.hpp>
#include <boost/fusion/include/std_pair.hpp>
#include <boost/fusion/include/map.hpp>
#include <boost/fusion/adapted/struct.hpp>
#include <boost/spirit/include/phoenix_core.hpp>
#include <boost/spirit/include/phoenix_operator.hpp>
#include <boost/spirit/include/phoenix_stl.hpp>


#include "stbl/stbl.h"
#include "stbl/HeaderParser.h"
#include "stbl/Article.h"
#include "stbl/logging.h"
#include "stbl/utility.h"

using namespace std;
using namespace boost;
namespace qi = boost::spirit::qi;

namespace stbl {

template <typename Iterator, typename Skipper = qi::ascii::blank_type>
struct HeaderGrammar: qi::grammar <Iterator, ::stbl::HeaderParser::header_map_t(), Skipper> {
    HeaderGrammar() : HeaderGrammar::base_type(header_lines, "HeaderGrammar Grammar") {
        field_key    = +qi::char_("0-9a-zA-Z-");
        field_value  = +~qi::char_("\n");
        header_lines = *(field_key >> *qi::lit(' ') >> ':' >> field_value >> qi::lexeme["\n"]);
    }

  private:
    qi::rule<Iterator, ::stbl::HeaderParser::header_map_t(), Skipper> header_lines;
    qi::rule<Iterator, std::string()> field_key, field_value;
};

namespace  {

std::string removeCommentLines(std::string input) {
    size_t pos = 0;
    while ((pos = input.find('#', pos)) != std::string::npos) {
        size_t endPos = input.find('\n', pos);
        if (endPos != std::string::npos) {
            input.erase(pos, endPos - pos + 1);
        } else {
            // If no newline character is found, erase till the end of the string
            input.erase(pos);
        }
    }
    return input;
}

}

class HeaderParserImpl : public HeaderParser
{
public:
    using it_t = std::string::const_iterator;

    HeaderParserImpl()
    {
    }

    void Parse(Article::Header& header, std::string& headerBlock) override {
        auto hdr = removeCommentLines(headerBlock);
        HeaderGrammar<it_t> grammar;
        auto iter = hdr.cbegin();
        auto end = hdr.cend();
        header_map_t headers;
        bool result = phrase_parse(iter, end, grammar, qi::ascii::blank, headers);

        if (!result || (iter != end)) {
            std::string::const_iterator some = iter+30;
            std::string context(iter, (some>end)?end:some);
            LOG_ERROR << "Parsing failed at: << \": " << context << "\"";
            throw runtime_error("Parse error");
        }

        LOG_TRACE << "Dumping headers: ";
        for (const auto &h : headers) {
            LOG_TRACE << "  '" << h.first << "' --> '" << h.second << "'";
        }

        Assign(header, headers);
    }

private:
    void Assign(Article::Header& hdr, const header_map_t headers) {

        hdr.uuid = Get("uuid", headers);
        hdr.title = GetWide("title", headers);
        hdr.tags = GetWideList("tags", headers);
        hdr.updated = GetTime("updated", headers);
        hdr.abstract = Get("abstract", headers);
        hdr.tmplte = Get("template", headers);
        hdr.type = Get("type", headers);
        hdr.menu = GetWide("menu", headers);
        hdr.banner = Get("banner", headers);
        hdr.banner_credits = Get("banner-credits", headers);
        hdr.comments = Get("comments", headers);
        hdr.have_uuid = !hdr.uuid.empty();
        hdr.have_updated = hdr.updated != 0;
        hdr.have_title = !hdr.title.empty();

        if (auto part = Get("part", headers); !part.empty()) {
            try {
                hdr.part = stoi(part);
            } catch(const std::exception& ex) {
                LOG_WARN << "Failed to cast part: " << part << " to integer.";
            }
        }

        string pri = Get("sitemap-priority", headers);
        if (!pri.empty()) {
            hdr.sitemap_priority = stoi(pri);
        }
        hdr.sitemap_changefreq = Get("sitemap-changefreq", headers);


        if (hdr.uuid.empty()) {
            hdr.uuid = CreateUuid();
        }

        auto published = Get("published", headers);

        if (!published.empty()) {
            if ((published == "false") || (published == "no")) {
                hdr.is_published = false;
            } else {
                hdr.published = GetTime("published", headers);
                hdr.have_published = true;
            }
        }

        hdr.expires = GetTime("expires", headers);
        hdr.authors = GetList("authors", headers);
        auto author = Get("author", headers);
        if (!author.empty()) {
            hdr.authors.insert(hdr.authors.begin(), author);
        }
    }

    std::string Get(
            const std::string& key,
            const header_map_t& headers) {
        auto it = headers.find(key);
        if (it == headers.end()) {
            return {};
        }

        return it->second;
    }

    std::wstring GetWide(
            const std::string& key,
            const header_map_t& headers) {

        return converter.from_bytes(Get(key, headers));
    }

    template <typename Iterator>
    void parse_list(Iterator first, Iterator last, std::vector<string>& v)
    {
        namespace phoenix = boost::phoenix;
        namespace ascii = boost::spirit::ascii;

        using qi::double_;
        using qi::phrase_parse;
        using qi::_1;
        using ascii::space;
        using phoenix::push_back;

        qi::rule<Iterator, std::string()> token = +~qi::char_(",");

        bool r = phrase_parse(first, last,
            (
                token[push_back(phoenix::ref(v), _1)] % ','
            )
            ,
            space);

        if (first != last) { // fail if we did not get a full match
            std::string::const_iterator some = first+30;
            std::string context(first, (some>last)?last:some);
            LOG_ERROR << "Parsing failed at: << \": " << context << "\"";
            throw runtime_error("Parse error");
        }
    }

    std::vector<std::string> GetList(const std::string& key,
                                      const header_map_t& headers) {
        auto it = headers.find(key);

        std::vector<string> list;

        if (it != headers.end()) {
            parse_list(it->second.begin(), it->second.end(), list);
        }

        return list;
    }

    std::vector<std::wstring> GetWideList(const std::string& key,
                                          const header_map_t& headers) {

        std::vector<std::string> list;
        list = GetList(key, headers);

        std::vector<std::wstring> wlist;
        for (const auto &v : list) {
            wlist.push_back(converter.from_bytes(v));
        }

        return wlist;
    }

    time_t GetTime( const std::string& key, const header_map_t& headers) {
        auto value = Get(key, headers);
        if (value.empty()) {
            return 0;
        }

        istringstream ss(value);
        tm t = {};
        ss >> std::get_time(&t, "%Y-%m-%d %H:%M");
        if (ss.fail()) {
            LOG_ERROR << "Failed to parse date: '" << value << "'";
            throw runtime_error("Parse error");
        }

        auto local = mktime(&t);

        t = {};
        gmtime_r(&local, &t);
        return mktime(&t);
    }

    std::wstring_convert<std::codecvt_utf8_utf16<wchar_t>> converter;
};

std::unique_ptr<HeaderParser> HeaderParser::Create() {
    return make_unique<HeaderParserImpl>();
}

}
