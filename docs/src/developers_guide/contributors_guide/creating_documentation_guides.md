[//]: # (Author: Jamie Melton)
[//]: # (Last Updated: 09/12/2025)

# Creating Documentation

As an organisation, we want Conjure Oxide to be thoroughly documented so that new members of our growing team can integrate smoothly. Whether you are currently on a project or not, contributing to our documentation is essential to achieving this goal.

To make creating and implementing documentation as smooth as possible, we ask that you follow the workflow as outlined below.

## Documentation Work Flow

1. Identify the documentation you will write.

   In many cases, documentation contributors will be writing about projects they are currently working on or have already completed.
However, there are still plenty of opportunities to contribute to documentation, even if you are not actively involved in a project or are seeking a small side project.
In such cases, please refer to [issue #1334](https://github.com/conjure-cp/conjure-oxide/issues/1334), which tracks all documentation writing tasks.
All unassigned child issues are available for anyone to work on!

   You may come across instances where important or useful documentation is missing from this book. If you notice such gaps and would like to address them as a side project, we encourage you to do so!

2. If the issue is not already present, open a child issue on our documentation tracking issue [#1334](https://github.com/conjure-cp/conjure-oxide/issues/1334).

    Make the title of this issue the name of the documentation you will be writing.

3. Link your child issue to a pr request.

   #### File type and naming convention

   All documentation for the Conjure Oxide book should be written in markdown. If you're unfamiliar with markdown or need a refresher on its syntax, we recommend reviewing [this guide](https://www.markdownguide.org/getting-started/).

   When naming your documentation file, match the file name as closely as possible to the heading of the documentation and use underscores ('_') to replace spaces. For example, if your documentation page is titled "Creating Documentation," name the file `creating_documentation.md`.

   #### Where to place documentation

   Place all documentation files in an appropriate location within the `/docs/src` directory. Each markdown file should be stored in the directory that matches its section in the documentation structure. For example, this file is located at `/docs/src/developers_guide/contributors_guide/`.

   If you know where your documentation belongs in the book, please place it in the appropriate directory. However, it's not required to know the exact location—moving files is quick and easy. If you're unsure where your documentation should go, place the file in `/docs/src/misc` so it can be relocated later to a more suitable section.

   #### Viewing documentation on the book

    You must ensure that the documentation you write formats as expected in the book. To do so, follow these steps:

    - Open `/docs/src/SUMMARY.md`.
    - If you have a definite location for your documentation, link the documentation in the appropriate section using the following format: `[ Section title ](path/to/file)`.
    - If you are unsure where your documentation should reside, place the documentation at any location, **but remember to delete this link once you're happy with your document's formatting**.
    - In your terminal, ensure that you are in the `/docs` directory and type in the following command `mdbooks serve --open`

   > **Important**
   >
   > Be sure to assign yourself and any other contributors working on the documentation to your child issue and pull request. This helps us keep track of who is responsible for each piece of documentation and ensures proper assignment.

4. Once you are happy with your documentation, request a review from one or more other members of your team.

   > **Important**
   >
   > Place any contributors to the documentation and the date where the documentation was last updated in a comment on the first and second lines of the file.
   > For instance:
   >
   > `[//]: # (Author: Jamie Melton)`
   >
   > `[//]: # (Last Updated: 09/12/2025)`
   >
   > has been placed at the top of this file.

5. When your team is happy with this documentation, request a review from the current book editors.

    The current book editors are: *JamieASM* and *HKhan-5*.

6. Assuming no adjustments are required, the documentation will then be merged in, and the child issue will be closed.

---

# What is Vale? How does Conjure-Oxide ensure that our documentation is high quality and consistent?

[Vale](https://vale.sh/docs/) is a prose linter. Think of it like `clippy`, but for documentation style, spelling, terminology, and consistency.

At a high level, Vale works by:

1. Reading a configuration file (`.vale.ini`)
2. Loading one or more styles/rules
3. Scanning matching files (Markdown in our case)
4. Emitting alerts (`suggestion`, `warning`, `error`)

In this repository, Vale is primarily used for documentation quality checks under `docs/`.

## How this repository configures Vale

Our project config lives in [`/.vale.ini`](../../../../.vale.ini) and currently contains:

- `StylesPath = tools/vale_styles`
- `Vocab = conjure_vocab`
- `[*.md]` + `BasedOnStyles = Conjure`

What this means:

- **StylesPath** tells Vale where our local style definitions and vocabulary live.
- **Vocab** enables the project vocabulary at:
  - `tools/vale_styles/config/vocabularies/conjure_vocab/accept.txt`
- **BasedOnStyles = Conjure** enables the rules in our `Conjure` style for Markdown files.

For vocabulary behavior and format details, see Vale’s vocabulary docs: [Vocabularies](https://vale.sh/docs/keys/vocab).

## Fixing PR lint failures by updating the dictionary

When Vale flags a term that is valid for this codebase (domain-specific term, acronym, tool name, etc.), add it to:

- `tools/vale_styles/config/vocabularies/conjure_vocab/accept.txt`

### File format rules

`accept.txt` supports one regex entry per line.

- Lines beginning with `#` are comments.
- Entries are regex patterns.
- Case sensitivity matters unless you explicitly make a pattern case-insensitive.

Examples:

- Case-insensitive whole term: `(?i)API`
- Character-class style: `[Jj]son`
- Optional suffix: `Biplates?`

### Practical workflow for a failing PR

1. Read the Vale error in the PR checks.
2. Decide whether the word should be:
   - corrected in the doc text, or
   - accepted as project vocabulary.
3. If it should be accepted, add a new entry to `accept.txt`.
4. Keep the entry as narrow/specific as possible to avoid false positives.
5. Commit and push; CI will re-run automatically.

### When *not* to add something to `accept.txt`

Do **not** add entries that are just typos or inconsistent wording.

The vocabulary should represent **intentional project terminology**, not bypass style checks globally.

Good candidates:

- Domain terms (`SATInt`, `Uniplate`, `Savile`)
- Tool names (`rustc`, `Valgrind`)
- Project-specific identifiers (`conjure_essence_parser`)
- Variable names from the code that you might need to refer to.

Bad candidates:

- Accidental misspellings
- One-off casing mistakes that should be corrected in source text

## Still unsure?

If you have any questions or concerns, please post them on the documentation discussion board. A book editor will respond to you promptly.
Likewise, if you have suggestions for improving or streamlining this process, feel free to share your ideas with the book editors! Your feedback is always welcome.
