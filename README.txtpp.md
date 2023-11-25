TXTPP#tag MAGOO
TXTPP#include magoo.txt
# MAGOO magoo

![Build Badge](https://img.shields.io/github/actions/workflow/status/Pistonite/magoo/rust.yml)
![Version Badge](https://img.shields.io/crates/v/magoo)
![License Badge](https://img.shields.io/github/license/Pistonite/magoo)
![Issue Badge](https://img.shields.io/github/issues/Pistonite/magoo)

**In Development. commands left are: update, remove**

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
To include MAGOO in your rust application, add it as a depedency. See https://docs.rs/magoo for more info.

TXTPP#tag MAGOO
TXTPP#include magoo.txt
## Use MAGOO

TXTPP#tag MAGOO
TXTPP#include magoo.txt
MAGOO runs `git` commands using sub-processes, so you must have `git` installed on the system.
You can run `magoo status --git` to check what version you have on your system, as well as
TXTPP#tag MAGOO
TXTPP#include magoo.txt
the officially supported `git` versions. Unsupported versions might work as well, MAGOO just doesn't know.


### Add a submodule

The argument for adding a submodule is very similar to [`git submodule add`](https://git-scm.com/docs/git-submodule#Documentation/git-submodule.txt-add-bltbranchgt-f--force--nameltnamegt--referenceltrepositorygt--depthltdepthgt--ltrepositorygtltpathgt)

TXTPP#tag MAGOO
TXTPP#include magoo.txt
MAGOO needs to know the following to add a submodule.:

|Arg|Description|Default|
|-|-|-|
|`URL`| The git URL like `https://github.com/owner/repo`. | URL is Required |
|`PATH`| The path in your repo the module should be at | Directory at the top level with the same name as the submodule repo|
|`BRANCH`| The branch to update to when you run `magoo update` | None (`HEAD`) |
|`NAME`| Name to identify the submodule for other commands | same as `PATH` |

It's recommended to always specify the `BRANCH`. Git by default will use the `HEAD` branch, which
is usually not what you want.

```bash
magoo install URL --branch BRANCH
magoo install URL PATH --branch BRANCH
magoo install URL PATH --branch BRANCH --name NAME --depth DEPTH --force
```

Run `magoo install help` to see other options

### Initialize the submodules
```bash
magoo install
```
TXTPP#tag MAGOO
TXTPP#include magoo.txt
MAGOO will ensure the submodules are cloned/updated to the commit stored in the index.
You should run `magoo install` every time you pull the changes from others, in case they were updated.
It also deletes submodules that are deleted by others (by running `status --fix --all`, see below).

### Show submodule status
```bash
magoo status [--long] [--all] [--fix]
```
TXTPP#tag MAGOO
TXTPP#include magoo.txt
Shows everything MAGOO knows about submodules in the current repo.

TXTPP#tag MAGOO
TXTPP#include magoo.txt
The `--fix` option will bring the submodule states back to a consistent state that MAGOO likes.
The state could be inconsistent if the git files were changed manually or by running
individual `git` commands, or by a remote change.

The `--all` option can potentially find more residues.

### Update submodules
TXTPP#tag MAGOO
TXTPP#include magoo.txt

MAGOO updates the submodule by fetching and checking out the latest updates from the remote, tracked by
the `BRANCH` specified when you add it.

- Update all submodules to the latest
   ```bash
   magoo update
   ```
- Update one submodule to the latest
   ```bash
   magoo update NAME
   ```
- Change the branch and/or URL of a submodule
   ```bash
   magoo update NAME --branch BRANCH --url URL
   ```

### Remove submodules
TXTPP#tag MAGOO
TXTPP#include magoo.txt
MAGOO can remove a submodule with ease:
```bash
magoo remove NAME
```

TXTPP#tag MAGOO
TXTPP#include magoo.txt
Note: Newer versions of git lets you delete a submodule with `git rm`. However, it doesn't delete the content in
`.git/modules`. MAGOO deletes those.
