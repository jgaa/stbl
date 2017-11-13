
#include <iostream>
#include <fstream>
#include <iomanip>

#include <boost/filesystem.hpp>

using namespace std;

int main(int argc, char *argv[]) {

    if (argc < 4) {
        cerr << "Usage: mkres namespace resource-name impl-file.cpp header-file.h file [file ...]" << endl;
    }

    const string ns = argv[1];
    const string res_name = argv[2];

    ofstream impl(argv[3]);
    ofstream hdr(argv[4]);
    int count = 0;


    hdr << "#include <map>" << endl
        << "#include <string>" << endl << endl
        << "namespace " << ns << '{' << endl << endl
        << "using resource_t = std::map<std::string, const char * const>;" << endl
        << "extern const resource_t " << res_name << ';' << endl
        << '}' << endl;

    impl << "#include \"" << argv[4] << '"' << endl
        << "namespace " << ns << '{' << endl << endl
        << "const resource_t " << res_name << " = {" << endl;

    for(int i = 5; i < argc; i++) {
        const boost::filesystem::path path = argv[i];

        if (!boost::filesystem::is_regular(path)) {
            clog << "Skipping " << path << endl;
            continue;
        }

        clog << "Processing: " << path << endl;

        ifstream in(path.c_str(), ios_base::in | ios_base::binary);

        if (count++) {
            impl << ',';
        }

        impl << endl << "// From " << path << endl
            << "{{\"" << path.filename().string() <<"\"}, {" << endl << "\"";

        while(true) {

            const unsigned char ch = static_cast<unsigned char>(in.get());
            if (!in) {
                break;
            }

            if (ch == '\r') {
                continue;
            }

            if (ch == '\n') {
                impl << "\\n\"" << endl << "\"";
            } else if (ch == '"') {
                impl << "\\\"";
            } else if (ch == '\\') {
                impl << "\\\\";
            }
            else if ((ch < ' ') || ch >= 127) {
                impl << "\\0x" << std::hex << std::setw(2) << std::setfill('0')
                    << static_cast<unsigned>(ch)
                    << std::setw(0) << std::dec;
            } else {
                impl << ch;
            }
        }

        impl << "\"}}" << endl;
    }

    impl << "};" << endl << "}" << endl;
}
