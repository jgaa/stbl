---
published:  2017-09-29 12:00
author:     jgaa
tags  :     blog, c++, publishing
abstract:   This is really exiting stuff!
image:      images/cute-cat.jpg
---
I just realized that what the world need most of everything, right *now*, is a new Static Blog Generator! So here we go!

# Content

In the articles folder, you can organize articles in a mix of:

- Textfiles in the articles folder itself. These are interpreted as normal blog posts.

- Textfiles in sub-folders, with names starting with an underscore. These are also
  interpreted as normal blog posts. The folders are just a convenience for you to
  organize your posts. Fore example by subject, year, year+month, or any other
  association that makes sense to you. You can add subdirectories like these in as
  many levels as you wish.

- Textfiles in sub-folders *not* starting with an underscore. These are interpreted as a series of related posts. The name of the directory will be listed on your main page, and the posts will be listed in chronological order. (RSS feeds will list the newest articles first).

# The header section

- author: The email address to the author of the article. If both authors and author is specified, the author in the 'author' field will appear first among the ahthors on the generated page.
- authors: A comma-separated listt of email addresses to authors.
- published: When the article was published: A date in 'YYYY-MM-DD HH:MM' format, or 'no' or 'false' if the article is unpublished. If the value is unset, the system will fall back to the file-date for the article. If the date is set to the future, the article will beheld back1.
- updated: When the article was last updated. A date in 'YYY-MM-DD HH:MM' format. If unset, the system will fall back to the file-date for the article.
- expires: When the article expired. A date in 'YYY-MM-DD HH:MM' format. If unset, the article will not expire. Expired articles are not published.