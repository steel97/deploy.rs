# deploy.rs
[![Build](https://github.com/steel97/deploy.rs/actions/workflows/build.yaml/badge.svg)](https://github.com/steel97/deploy.rs/actions/workflows/build.yaml)

simple deployment tool for debian based target servers, this tool uses sha1sum command to compute difference between files, pack them to tgz archive and upload it on target server.

usage:
```
cargo run <config_file>
```
Example configs: [example.json](example.json), [example_cert.json](example_cert.json)

build:
```
cargo build --release
```

server requirements:
```
+ debian based OS
+ mktemp
+ sha1sum
+ tar
+ rm
```

supported keys:
```
ssh-ed25519 (with password only)
```