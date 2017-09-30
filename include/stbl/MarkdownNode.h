#pragma once

#include <string>
#include <iostream>
#include <memory>
#include <deque>

namespace stbl {


/*! Interface for a unit of markdown content
*/
class MarkdownNode
{
public:
    using ptr_t = unique_ptr<MarkdownNode>;

    enum class Type {
        ROOT,
        TEXT,
        PARAGRAPH,
        LIST,
        HEADLINE,
        FORMATTING,
        QUOTE,
        LINK,
        IMAGE,
        VIDEO
    };

    enum class Formatting {
        BOLD,
        ITALIC,
        UNDERLINE,
        STRIKEOUT
    };

    enum class List {
        ORDERED,
        UNORDERED,
        TASKS
    }

    MarkdownNode() = default;
    virtual ~MarkdownNode() = default;
    virtual Type GetType() = 0;

    void Add(ptr_t && child) {
        children_.push_back(std::move(child));
    }

    /*! Render this object and all of it's children. */
    virtual void Render(std::ostream& out) const = 0;

protected:
    void RenderClildren(std::ostream & out) const {
        for(const auto& c : children_) {
            c->render(out);
        }
    }

    std::deque<ptr_t> children_;
};

class MarkdownRoot : public MarkdownNode
{
public:
    MarkdownRoot(const std::string& text)
    : text_{text} {}

    stbl::MarkdownNode::Type GetType() override { return Type::ROOT; }

    void Render(std::ostream & out) const override {
        RenderClildren(out);
    }

};

class MarkdownText : public MarkdownNode
{
public:
    MarkdownText(const std::string& text)
    : text_{text} {}

    stbl::MarkdownNode::Type GetType() override { return Type::TEXT; }
    void Render(std::ostream & out) const override {
        out << text_;
    }

private:
    const std::string text_;
};

class MarkdownHeadline : public MarkdownNode
{
public:
    MarkdownHeadline(unsigned level)
    : level_{level} {}

    stbl::MarkdownNode::Type GetType() override { return Type::HEADLINE; }
    void Render(std::ostream & out) const override {
        out << "<H" << level_ << '>';
        RenderClildren(out);
        out << "</H" << level_ << '>';
    }

private:
    const usnigned level_;
};

}

