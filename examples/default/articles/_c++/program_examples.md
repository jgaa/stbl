---
authors: jgaa
tags: c++, programming, example
---
Just some code to test / show code blocks.

Note that this post is contained in a directory starting with underscore. 
This is for convenience, to let you group together related content in
folders. The post itself will be independent. 

## Extract code blocks

Here is some code used to extract the code-blocks from the markup:
```
    while(true) {
        const auto pos = content.find("```");
        if ((pos != content.npos) && (content.size() >= (pos + 7))) {
            const auto spos = content.find('\n', pos);
            const auto epos = content.find("```\n", pos + 4);
            if (epos != content.npos) {
                // pos = start ```
                // spos = end of start-line
                // epos is end ```
                string code_block = R"(<pre class="code">)"s
                    + content.substr(spos, epos - spos)
                    + "</pre>\n"s;
                content.replace(pos, (epos + 4) - pos, code_block);
            }
        } else {
            break;
        }
    }
```
Done.

## How to eat headers...
```c++
void EatHeader(std::istream& in) {

        int separators = 0;

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
```

And that's it It I guess...
