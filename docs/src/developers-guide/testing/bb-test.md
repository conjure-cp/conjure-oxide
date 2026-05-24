# Testing in Conjure Oxide


### Integration Test

The Integration Test setup takes one essence file and checks that every step of solving works correctly by comparing the output of each step with files that contain previously prepared 'outputs', which have been verified for correctness. It checks the parsed output, the application of rules in the rule engine and the solutions, thus verifying each of the steps of the solution process.

New Integration Tests should be added whenever additions are made to the AST, more solver support, new solving features, new representations etc. 

## The Black-Box Testing Setup

So far, testing of the compiled conjure oxide tool (also sometimes called Black-Box Testing[^5]) has been done using the GNU Parallel command line tool, and in certain cases using python or bash scripts. For a full explanation of the tool's many useful features, take a look at the (incredibly comprehensive) documentation for the tool on GNU's webpage. 

For the purposes of this chapter, it is important to know that GNU Parallel is a useful command line tool which, true to its name, is used to run commands in parallel. In addition to this, the tool can put together the commands which are to be run, using what is essentially a cartesian product of lists which are passed to it as arguments. It can also manage per-process runtime, memory allocation, process usage and so on, using command line flags. 

The Conjure Oxide testing setup (in the conjure-cp/conjure-oxide-tester repository) uses GNU parallel with a layer of abstraction built in python which records the time comparisons into a database. The Repository also includes tools that allow some rudimentary examination and visualisation of the runtime data that has been collected and placed in the output database. 

## Using the Tester and Associated Tools

### Prerequisites

- Python 3.14+ (project pyproject.toml requires >=3.14).
- GNU Parallel (command line: `parallel`) for `runner/run_tests.sh`
- SQLite3 (for the results Database)
- runsolver: used to manage an environment for the solver tool. This tool is the standard in SAT solver competitions, and other such applications
- Optional, but highly recommended: `uv` for a project manager and `ty` for a type checker (used by some workflows)
- In case `uv` is not used, look at pyproject.toml for an up-to-date list of the python dependencies. Install each of these using pip _inside_ the virtual environment (for various reasons, your linux distribution may not allow system-wide installation).

Create and activate a virtual environment, then install these dependencies using the following commands:

```bash
python3 -m venv .venv
source .venv/bin/activate
python -m pip install --upgrade pip
python -m pip install uv pandas textual ty
```

### Configuration

- `settings.json` contains runner command mappings and the output DB path. Example keys:
	- `runner_commands`: internal JSON object whose keys are the names of the 'runners' and values are the command with which those 'runner names' are associated. You can use this to effectively define a short alias for a long command. The command can be pretty much whatever you need it to be, including flags and so on. 
	- `outfile`: relative path to the SQLite Database, where all of the data that is collected by `timer.py`.
    - `runsolver_cfg`: internal JSON object which takes in the resource configuration to allocate internally to the `runsolver` tool.
        - `memory`: The used memory allocation allowed per solver. 
        - `walltime`: The used maximum runtime per solver, the process will terminate after this amount of time.
        - `cpus`: The CPUs allocated per solver 

Edit `settings.json` to add or modify runners and change the configuration with which to run `runsolver`.

### Using Different Solver Configurations: 

The `runner_commands` header in the settings allows for the addition of a commands to benchmark. The underlying system does not try to parse these commands and simply runs them in a shell process, which means these commands can be anything that can be run on the command line. For example, it can be as simple as the following configuration:
```json
"runner_commands": {
    "conjure": "conjure solve",
    "conjure_sat": "conjure solve --solver cadical",
    "oxide_main_sat": "conjure-oxide solve -s sat",
  },
```
which provides 'runners' for different solvers: plain conjure, conjure with some flags and conjure-oxide with some flags.

Another useful example is the following setup:
```json
"runner_commands": {
    "naive": "conjure-oxide solve --rewriter naive",
    "morph": "conjure-oxide solve --rewriter morph",
  },
```
which provides 'runners' that both use the same solver (default is minion) but use different rewriters.

It can also be as complicated as this configuration:
```json
  "runner_commands": {
    "main": "conjure-oxide solve",
    "foo": "conjure-oxide_branch-foo solve",
    "hash_bf144c": "conjure-oxide_headless-bf144c solve"
  },
```
All three configurations are using conjure-oxide with all default options. However, they use executables compiled from different checkouts on the git repository:
- `main` provides the most up-to-date stable version
- `foo`  is the version of conjure-oxide that lives on the branch called `foo`
- `hash_bf144c` provides the version in the headless state, when the repository is at the state which it was in during the commit with hash "bf144c". 

This would need some extra setup aside from simply adding the commands into the settings.json file. The user would also need to make sure that they have switched to these branches, compiled conjure-oxide _on those branches_ and stored the executables in their `$PATH` with the appropriate names, which can then be used in the `"runner_commands"` json object.  

### Running Tools

- **setup.py**: Re-initialise the database at the path from the configuration. To (re)create the Database used by the runner, run `python3 src/setup.py`. This script will prompt for confirmation and will wipe the existing database if confirmed. If no database exists, it will create one.
- **show.py**: This will print out the database in the form of a database. By itself, this is not very useful but it is good to know the status of your data collection run.
- **timer.py**: This tool will take a runner name and an essence file as CLI arguments. It will then use the runner passed in to generate a solution for the file passed in a record a bunch of information, including the runtime and the number of clauses and so on. 
- **view.py**: This is a TUI tool to look through the result stored in the database at the path in the `settings.json` file. It uses the `textual` package.


### Running the Convenience Script

The main runner script is `runner/run_tests.sh`.

Usage examples:

Run with a single runner defined in settings.json
```bash
./runner/run_tests.sh oxide_main_minion
```
Run with a runner and filter models that contain 'max'
```bash
./runner/run_tests.sh oxide_main_sat max
```

Disable clauses collection
```bash
./runner/run_tests.sh --no-closures oxide_main_sat
```

`run_tests.sh` finds all `.essence` files under `models/`, filters them by an optional operand string, and runs them in parallel using the runner commands from `settings.json`. Results and failures are written to the configured SQLite DB. If a run exits non-zero, its runtime is recorded as `-1.0` and the error is logged to the `failures` table.

## Using the Container

For unfamiliar operating systems or machines, it is best to run the testing setup inside a Docker/Podman container, as it will circumvent the need for administrator permissions and so on. For this purpose, a containerfile has been provided in the `conjure-oxide-tester` repository which provides much of the support that is needed for this application.

Podman is used for the purposes of these instructions. However, docker syntax is identical to podman. On many machines, docker even aliases down to podman. 
 
### Build An Image

From the root directory of `conjure-oxide-tester`, build a podman image with a name that can then be referenced later on. The `-t` flag allows the container to be named by the user:

```bash
podman build -t test-container .
```
 
### Create a Directory on The Host
 
This is where the database will live after the container is gone:
 
```bash
mkdir ~/my-container-output
```
 
### Run the Container with a Bind Mount
 
When a container is removed, everything written inside it is lost, unless a directory on the host machine is mounted to the container at run time. The `-v` flag allows some external directory on the host to be mounted into the container:
 
```bash
podman run -it --rm -v ~/my-container-output:/output my-image
```

Now, inside the container, make sure that the "outfile" in settings.json is set to somewhere inside the /output directory in the container.

After this intial setup step, each of the tools can be used in the usual manner from within the container. 

---

[^1]: Check out the Rust Book's documentation.
[^2]: Primarily, this added functionality uses integration testing, which uses the external interface of the crate. Check out the page in Rust Book here. 
[^3]: This refers to Rust's provided advanced testing, which is not used by conjure oxide as of now
[^4]: which is a file name 'build.rs' in the root directory of the crate.
[^5]: 'Black-Box Testing' is a model of testing in Software Engineering, which refers to tests which are made in a way that presumes no knowledge of the system being tested. 