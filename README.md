## Laks & Vjerdha - SyncRs (Rsync clone in rust)

### Students

- Merijn Laks <MerijnLaks@protonmail.com>
- Gejsi Vjerdha <gejsi.vjerdha@ip-paris.fr>

### Repository

- https://gitlab.telecom-paris.fr/net7212/2425/project-laks-vjerdha
- `git@gitlab.enst.fr:net7212/2425/project-laks-vjerdha.git`

### Description

This project aims to improve the memory safety of Rsync (<https://linux.die.net/man/1/rsync>), an existing and much-used tool for file syncing and archiving on the GNU/Linux operating system.
The clone will implement many of the original features, such as incremental updates and remote file syncing, as well as quality-of-life flags like archiving.

### Core Features

- local copying of files in chunks with options for directories, permissions, and other metadata.
- syncing of files to remote targets
- incremental updates through a custom delta-transfer protocol
- custom rolling hashing scheme

### Challenges

- implementing all options from the reference application
- daemon programming
- handling of remote shells
- symlinks


### Getting Started
#### Verifying the program

verify the differences between `local-input` and `remote-input`
```Bash
  diff -r local-input remote-output
```

syn the contents `local-input` to `remote-output`
```Bash
  cargo run -- --debug local-input remote-output
```

verify the differences between `local-input` and `remote-input` after syncing
```Bash
  diff -r local-input remote-output
```

#### Build the project
```Bash
  cargo build
```
#### Local operation
For local operation, **syncrs** works similar to the `mv` command,
on top of using incremental updates.
```Bash
    ./target/debug/syncrs foo bar
```
will sync the file `foo` to the file `bar`

#### Remote operation
```Bash
    ./target/debug/syncrs foo user@host:bar
```
will sync the file `foo` to the file `bar` in the home
directory of `user` on `host`

##### Start a server
```Bash
    ./target/debug/syncrs --server <SERVER_IP>
```
the default server ip is **127.0.0.1**

> for additional information, see the --help flag

### Running unit tests
In the project root, run
```Bash
    cargo test
```

### Running fuzzing tests
In the project root, run
```Bash
  cargo fuzz run fuzz_compute_delta
```
This is the only function of which it would make sense
to use fuzzy testing, since our `compute_deltas` function uniquely takes
a vector of bytes as an input.