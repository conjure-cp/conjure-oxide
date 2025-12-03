# Creating Documentation

As an organisation, we want Conjure Oxide to be as well documented as possible so that new members of our growing team can integrate themselves as smoothly as possible. Whether you are already on a project or not, contributing to our documentation is vital to achieve this goal.

To make creating and implementing documentation as smooth as possible, we ask that you follow the workflow as outlined below. 

## Documentation Work Flow

1. Identify the documentation you will write.

    In many cases, contributors of documentation will be documenting projects that they are currently working on and/or have completed working on. That being said, there are still many opportunities to contribute to documentation if you are currently not on a project or looking for a small side project. In this case, please refer to [issue #1334](https://github.com/conjure-cp/conjure-oxide/issues/1334) which tracks all documentation writing issues. All unassigned child issues are free for anyone to contribute to! 

    There may also be a case where you discover that important or useful documentation is currently not present in this book. If you find this to be the case and would like to take this documentation on as a side project, we welcome you do to so!

2. If the issue is not already present, open a child issue on our documentation tracking [issue #1334](https://github.com/conjure-cp/conjure-oxide/issues/1334)

    Make the title of this issue the documentation you will be writing.

3. Link your child issue to a pr request
    #### File type and naming convention

    All documentation written for the Conjure Oxide book uses markdown. If you are unsure about markdown and its syntax, we reccommend that you take time to read [this guide](https://www.markdownguide.org/getting-started/). 

    When naming the file, we ask that you keep the name as close as possible to the heading of the documentation and use '_' to represent whitespace. For instance, the title of this documentation page is 'Creating Documentation', so the files name is 'creating_documentation.md'.

    #### Where to place documentation

    All documentation should be placed at some location inside of the `/docs/src` directory. This directory is set up such that each markdown file is in a directory that corresponds to the section it is found in. For instance, this file is located in `/docs/src/developers_guide/contributors_guide/`. 

    If you know where your documentation should go in the book, we ask that you place it there. That being said, there is no requirement for you to know exactly where your documenation should live - moving documentation around is very quick and simple to do. If you are uncertain where your documentation should live, we ask that you place the file in `/docs/src/misc` that way the documentation can be moved to a more appropriate spot later.

    #### Viewing documenation on the book

    You must ensure that the documentation you write formats as expected in the book. To do so, follow these steps:

    - Open `/docs/src/SUMMARY.md`.
    - If you have a definite location for your documenation, link the documentation in the appropriate section using the following format: `\[ Section title \]\(path/to/file\)`.
    - If you are unsure where your documenation should reside, place the documentation at any location, **but remember to delete this link once you're happy with your documentations formatting**.
    - In your terminal, ensure that you are in the `/docs` directory and type in the following command `mdbooks serve --open`

4. Once you are happy with your documentation, request a review from one or more other members of your team.

5. When your team is happy with this documentation, request a review from the current book editors.

    The current book editors are: *JamieASM* and *HKhan-5*.

6. Assuming no adjustments are required, the documentation will then be merged in and the child issue will be closed.

---

## Still unsure?

If you have any questions or concerns, please do not hesitate to get into contact with the current book editors (JamieASM and HKhan-5). 

Furthermore, if you have any ideas on how this process can be improved or made smoother, feel free to share your ideas with them! 
