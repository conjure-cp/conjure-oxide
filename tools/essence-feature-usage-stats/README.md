# essence-feature-usage-stats

## About

This is an internal tool for the [conjure-oxide](https://github.com/conjure-cp/conjure-oxide) project.

It does the following:
- Given a directory containing Essence files, go through it and count how often every Essence language feature is used
- Display this data as a simple web page with a table

The purpose of this is to make it easier to find Essence examples to test specific Essence language features, which should be useful over the course of rewriting the conjure tool stack in Rust


## Usage

> This is heavily WIP and I am planning to make this more usable in the future.
> Eventually, this will be a Flask web server to host the web page, a better frontend (to make it searchable, sortable, etc), and the page will auto-update on changes to the relevant Essence example repos

For now, though:

- Put paths to conjure directory and Essence file directory in .env
- Run the main.py file
- See the generated HTML file


## ToDo

- [x] Get basic stats in JSON format
- [x] Get basic stats as HTML table
- [ ] Table sorting
- [x] Code refactoring
- [ ] Documentation
- [ ] Show/Hide table rows/columns
- [ ] Web server for hosting the HTML page
- 🏗️ GitHub Actions integration
- [x] Extra stats, e.g. file line count
- [ ] Sorting using these stats


------------------------------------------

- Georgii Skorokhod and Hannah Zheng, 2023
- University of St Andrews
- Developed as part of a Vertically Integrated Project by Ozgur Akgun et al
- (See [main repos](https://github.com/conjure-cp))
