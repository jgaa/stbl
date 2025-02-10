
#include <iostream>
#include <fstream>
#include <iomanip>

#include <filesystem>

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
        << "using resource_t = std::map<std::string, std::pair<const unsigned char *, size_t>>;" << endl
        << "extern const resource_t " << res_name << ';' << endl
        << '}' << endl;

    impl << "#include \"" << argv[4] << '"' << endl
        << "namespace " << ns << '{' << endl << endl
        << "const resource_t " << res_name << " = {" << endl;

    for(int i = 5; i < argc; i++) {
        const std::filesystem::path path = argv[i];

        if (!std::filesystem::is_regular_file(path)) {
            clog << "Skipping " << path << endl;
            continue;
        }

        clog << "Processing: " << path << endl;

        ifstream in(path.c_str(), ios_base::in | ios_base::binary);

        if (count++) {
            impl << ',';
        }

        impl << endl << "// From " << path << endl
            << "{{\"" << path.filename().string() <<"\"}, {" << endl
            << "reinterpret_cast<const unsigned char *>(" << endl << "\"";

        size_t bytes = {};
        size_t col = 1;

        while(true) {

            const unsigned char ch = static_cast<unsigned char>(in.get());
            if (!in) {
                break;
            }

            ++bytes;

            if (ch == '\n') {
                col = 1;
                impl << "\\n\"" << endl << "\"";
            } else if (ch == '\r') {
                ++col;
                impl << "\\r";
            } else if (ch == '"') {
                col += 2;
                impl << "\\\"";
            } else if (ch == '\\') {
                col += 2;
                impl << "\\\\";
            }
            else if ((ch < ' ') || ch >= 127) {
                col += 4;
                impl << "\\" << std::oct << std::setw(3) << std::setfill('0')
                    << static_cast<unsigned>(ch)
                    << std::setw(0) << std::dec;
            } else {
                ++col;
                impl << ch;
            }

            if (col >= 80) {
                impl << "\"" << endl << "\"";
                col = 1;
            }

        }

        impl << "\"), " << bytes << "}}" << endl;
    }

    impl << "};" << endl << "}" << endl;
}
