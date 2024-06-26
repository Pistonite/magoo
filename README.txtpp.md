TXTPP#tag MAGOO
TXTPP#include magoo.txt
# MAGOO magoo

![Build Badge](https://img.shields.io/github/actions/workflow/status/Pistonite/magoo/rust.yml)
![Version Badge](https://img.shields.io/crates/v/magoo)
![License Badge](https://img.shields.io/github/license/Pistonite/magoo)
![Issue Badge](https://img.shields.io/github/issues/Pistonite/magoo)

TXTPP#tag MAGOO
TXTPP#include magoo.txt
This MAGOO is Magoo, he helps you manage git submodules with ease, like `npm` or `cargo`, but for submodules.

Adding a submodule is easy, but have you ever struggled with:
- How do I update a submodule?
- How do I delete a submodule?
- How do I tell my colleagues how to update their submodules after I update them????

TXTPP#tag MAGOO
TXTPP#include magoo.txt
MAGOO saves all of us by running the `git` commands for us. He figures out
TXTPP#tag MAGOO
TXTPP#include magoo.txt
all the relevant information from the git repository itself. There's no extra files
required and MAGOO works out of the box for all git repos with submodules.

TXTPP#tag MAGOO
TXTPP#include magoo.txt
MAGOO **does not like pipelines**. Please simply let your pipeline
checkout the submodules (recursively if needed). For example, if you are using GitHub Actions:
```yaml
- uses: actions/checkout@v4
  with:
    submodules: recursive
```

TXTPP#tag MAGOO
TXTPP#include magoo.txt

## Install MAGOO

TXTPP#tag MAGOO
TXTPP#include magoo.txt
### MAGOO As a CLI tool
```
cargo install magoo
```

TXTPP#tag MAGOO
TXTPP#include magoo.txt
### MAGOO As a library
TXTPP#tag MAGOO
TXTPP#include magoo.txt
To add MAGOO as a dependency:
```
cargo add magoo
```
See https://docs.rs/magoo for more info.

TXTPP#tag MAGOO
TXTPP#include magoo.txt
## Use MAGOO

TXTPP#tag MAGOO
TXTPP#include magoo.txt
MAGOO runs `git` commands using sub-processes, so you must have `git` installed on the system.
TXTPP#tag MAGOO
TXTPP#include magoo.txt
By default, MAGOO checks if the `git` version is supported.
To print what version is supported manually, run:
```
magoo status --git
```

TXTPP#tag MAGOO
TXTPP#include magoo.txt
Unsupported versions might work as well, you can let MAGOO know with the `--allow-unsupported` flag (note
it needs to be before the subcommand)
```
magoo --allow-unsupported status
```


### Add a submodule
```bash
magoo install URL PATH
```
(`magoo install --help` to see a full list of options)

The arguments for adding a submodule is very similar to [`git submodule add`](https://git-scm.com/docs/git-submodule#Documentation/git-submodule.txt-add-bltbranchgt-f--force--nameltnamegt--referenceltrepositorygt--depthltdepthgt--ltrepositorygtltpathgt)

TXTPP#tag MAGOO
TXTPP#include magoo.txt
MAGOO needs to know the following to add a submodule.:

|Arg|Description|Default|
|-|-|-|
|`URL`| The git URL like `https://github.com/owner/repo`. SSH and relative URLs also work. See [`git submodule add`](https://git-scm.com/docs/git-submodule#Documentation/git-submodule.txt-add-bltbranchgt-f--force--nameltnamegt--referenceltrepositorygt--depthltdepthgt--ltrepositorygtltpathgt) for details | URL is required |
|`PATH`| The path in your repo the module should be at | Directory at the top level with the same name as the submodule repo|
|`BRANCH`| The branch to update to when you run `magoo update` | None (`HEAD`) |
|`NAME`| Name to identify the submodule for other commands | same as `PATH` |

It's recommended to always specify the `BRANCH`. Git by default will use the `HEAD` branch, which
is usually not what you want.

The submodule will not be cloned recursively when adding. If you need, run `magoo install` after the `add` to clone the recursive submodules.

### Initialize/pull the submodules
```bash
magoo install
```
TXTPP#tag MAGOO
TXTPP#include magoo.txt
MAGOO will ensure the submodules are cloned/updated to the commit stored in the index.
You should run `magoo install` every time you pull the changes from others, in case they were updated.
It also deletes submodules that are deleted by others (by running `status --fix`, see below).

By default, submodules are installed recursively, you can use `--no-recursive` to only install the ones specified by the top-level repo.

### Show submodule status
```bash
magoo status [--long] [--fix]
```
TXTPP#tag MAGOO
TXTPP#include magoo.txt
MAGOO will show you everything he knows about submodules in the current repo.

TXTPP#tag MAGOO
TXTPP#include magoo.txt
The `--fix` option will bring the submodule states back to a consistent state that MAGOO likes.
The state could be inconsistent if the git files were changed manually or by running
individual `git` commands, or by a remote change.

TXTPP#tag MAGOO
TXTPP#include magoo.txt
MAGOO will fix the state by either de-initializing the submodule (if possible), or delete the submodule.

### Update submodules
```bash
magoo update
```
TXTPP#tag MAGOO
TXTPP#include magoo.txt
This tells MAGOO to update all submodules to be sync with the remote `BRANCH` (specified when submodule was added).
See `magoo update --help` for more info.

You can also:
- Update one submodule to the latest
   ```bash
   magoo update NAME
   ```
- Change the branch and/or URL of a submodule
   ```bash
   magoo update NAME --branch BRANCH --url URL
   ```

### Remove submodules
```bash
magoo remove NAME
```
TXTPP#tag MAGOO
TXTPP#include magoo.txt
MAGOO will remove every trace of the submodule, with this single command.

TXTPP#tag MAGOO
TXTPP#include magoo.txt
Note: Newer versions of git lets you delete a submodule with `git rm`. However, it doesn't delete the content in
`.git/modules`. MAGOO deletes those as well.
