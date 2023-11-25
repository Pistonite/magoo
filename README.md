# ![magoo](https://raw.githubusercontent.com/Pistonite/magoo/main/magoo.webp) magoo

![Build Badge](https://img.shields.io/github/actions/workflow/status/Pistonite/magoo/rust.yml)
![Version Badge](https://img.shields.io/crates/v/magoo)
![License Badge](https://img.shields.io/github/license/Pistonite/magoo)
![Issue Badge](https://img.shields.io/github/issues/Pistonite/magoo)

This ![magoo](https://raw.githubusercontent.com/Pistonite/magoo/main/magoo.webp) is Magoo, he helps you manage git submodules with ease, like `npm` or `cargo`, but for submodules.

Adding a submodule is easy, but have you ever struggled with:
- How do I update a submodule?
- How do I delete a submodule?
- How do I tell my colleagues how to update their submodules after I update them????

![magoo](https://raw.githubusercontent.com/Pistonite/magoo/main/magoo.webp) saves all of us by running the `git` commands for us. He figures out
all the relevant information from the git repository itself. There's no extra files
required and ![magoo](https://raw.githubusercontent.com/Pistonite/magoo/main/magoo.webp) works out of the box for all git repos with submodules.

![magoo](https://raw.githubusercontent.com/Pistonite/magoo/main/magoo.webp) **does not like pipelines**. Please simply let your pipeline
checkout the submodules (recursively if needed). For example, if you are using GitHub Actions:
```yaml
- uses: actions/checkout@v4
  with:
    submodules: recursive
```


## Install ![magoo](https://raw.githubusercontent.com/Pistonite/magoo/main/magoo.webp)

### ![magoo](https://raw.githubusercontent.com/Pistonite/magoo/main/magoo.webp) As a CLI tool
```
cargo install magoo
```

### ![magoo](https://raw.githubusercontent.com/Pistonite/magoo/main/magoo.webp) As a library
To add ![magoo](https://raw.githubusercontent.com/Pistonite/magoo/main/magoo.webp) as a dependency:
```
cargo add magoo
```
See https://docs.rs/magoo for more info.

## Use ![magoo](https://raw.githubusercontent.com/Pistonite/magoo/main/magoo.webp)

![magoo](https://raw.githubusercontent.com/Pistonite/magoo/main/magoo.webp) runs `git` commands using sub-processes, so you must have `git` installed on the system.
To check the version info, run
```
magoo status --git
```

Unsupported versions might work as well, ![magoo](https://raw.githubusercontent.com/Pistonite/magoo/main/magoo.webp) just doesn't know.


### Add a submodule
```bash
magoo install URL PATH
```
(`magoo install --help` to see a full list of options)

The arguments for adding a submodule is very similar to [`git submodule add`](https://git-scm.com/docs/git-submodule#Documentation/git-submodule.txt-add-bltbranchgt-f--force--nameltnamegt--referenceltrepositorygt--depthltdepthgt--ltrepositorygtltpathgt)

![magoo](https://raw.githubusercontent.com/Pistonite/magoo/main/magoo.webp) needs to know the following to add a submodule.:

|Arg|Description|Default|
|-|-|-|
|`URL`| The git URL like `https://github.com/owner/repo`. SSH and relative URLs also work. See [`git submodule add`](https://git-scm.com/docs/git-submodule#Documentation/git-submodule.txt-add-bltbranchgt-f--force--nameltnamegt--referenceltrepositorygt--depthltdepthgt--ltrepositorygtltpathgt) for details | URL is required |
|`PATH`| The path in your repo the module should be at | Directory at the top level with the same name as the submodule repo|
|`BRANCH`| The branch to update to when you run `magoo update` | None (`HEAD`) |
|`NAME`| Name to identify the submodule for other commands | same as `PATH` |

It's recommended to always specify the `BRANCH`. Git by default will use the `HEAD` branch, which
is usually not what you want.

### Initialize/pull the submodules
```bash
magoo install
```
![magoo](https://raw.githubusercontent.com/Pistonite/magoo/main/magoo.webp) will ensure the submodules are cloned/updated to the commit stored in the index.
You should run `magoo install` every time you pull the changes from others, in case they were updated.
It also deletes submodules that are deleted by others (by running `status --fix`, see below).

### Show submodule status
```bash
magoo status [--long] [--fix]
```
![magoo](https://raw.githubusercontent.com/Pistonite/magoo/main/magoo.webp) will show you everything he knows about submodules in the current repo.

The `--fix` option will bring the submodule states back to a consistent state that ![magoo](https://raw.githubusercontent.com/Pistonite/magoo/main/magoo.webp) likes.
The state could be inconsistent if the git files were changed manually or by running
individual `git` commands, or by a remote change.

![magoo](https://raw.githubusercontent.com/Pistonite/magoo/main/magoo.webp) will fix the state by either de-initializing the submodule (if possible), or delete the submodule.

### Update submodules
```bash
magoo update
```
This tells ![magoo](https://raw.githubusercontent.com/Pistonite/magoo/main/magoo.webp) to update all submodules to be sync with the remote `BRANCH` (specified when submodule was added).
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
![magoo](https://raw.githubusercontent.com/Pistonite/magoo/main/magoo.webp) will remove every trace of the submodule, with this single command.

Note: Newer versions of git lets you delete a submodule with `git rm`. However, it doesn't delete the content in
`.git/modules`. ![magoo](https://raw.githubusercontent.com/Pistonite/magoo/main/magoo.webp) deletes those as well.
