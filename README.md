# ![Magoo](./magoo.webp)
This ![](./magoo.webp) is Magoo, the cat that manages `git submodule` commands for you.

![magoo](./magoo.webp) is friendly for people who are familiar with other package
manager, such as `npm` or `cargo`.

![magoo](./magoo.webp) **does not like pipelines**. Please simply let your pipeline
checkout the submodules (recursively if needed). For example, if you are using GitHub Actions:
```yaml
- uses: actions/checkout@v4
  with:
    submodules: recursive
```

## Install ![magoo](./magoo.webp)
### ![magoo](./magoo.webp) As a CLI tool
```
cargo install magoo
```
### ![magoo](./magoo.webp) As dependency
The `--no-default-features` flag will turn off the `cli` feature flag,
which reduces the dependency.
```
cargo add magoo --no-default-features
```

## Use ![magoo](./magoo.webp) in CLI
![magoo](./magoo.webp) runs `git` commands using sub-processes,
so you must have `git` installed on the system.

Run `magoo git` to see the minimum supported `git` version and if it is installed on your system.

Run `magoo help` to see the full list of commands.

![magoo](./magoo.webp) will not commit any changes, but some commands may require staging certain files.
Be sure to check `git status` afterwards.

### Add a dependency
(This is similar to `npm install <package>` or `cargo add <package>`)

To add a dependency, ![magoo](./magoo.webp) needs to know:
- `URL`: The git URL like https://github.com/owner/repo
- `PATH`: The path in your repo the module should be at

```bash
magoo install URL PATH
magoo install URL PATH --branch BRANCH --name NAME --depth DEPTH --force
```

Run `magoo install help` to see other options

### Install the dependencies
(This is similar to `npm install`)

Similar to how one would run `npm install` after pulling from remote to get the dependencies
updated to the version specified in `package.json`, ![magoo](./magoo.webp) can do that for you as well.
```bash
magoo install
```

### Update dependencies
(This is similar to `npm update` or `cargo update`)

![magoo](./magoo.webp) can also update the dependency to the latest version tracked by the branch specified
when the dependency was added (or `HEAD` if it was not specified).

- Update all dependencies to the latest
   ```bash
   magoo update
   ```
- Update one dependency to the latest
   ```bash
   magoo update NAME
   ```
- Update one dependency, and also change the branch and/or url
   ```bash
   magoo update NAME --branch BRANCH --url URL
   ```
### Remove dependencies
(This is similar to `npm uninstall` or `cargo remove`)

![magoo](./magoo.webp) understands your pain for removing submodules.
```bash
magoo remove NAME [--force]
```
`--force` will discard any changes made in the submodule (`git submodule deinit --force`)
