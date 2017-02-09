# pr_demon [![Build Status](https://travis-ci.org/lawliet89/pr_demon.svg?branch=master)](https://travis-ci.org/lawliet89/pr_demon)
A daemon to monitor pull requests (PR) from Bitbucket and trigger builds for the PR on Teamcity.

## Configuration
See `tests/fixtures/config.json` for an example configuration file.

## Usage
Run `cargo run --release -- path/to/config.json` or `cat path/to/config.json | cargo run --release -- -`

Alternatively, if you place the configuration file in `./config/config.json`, you can run the daemon in a Docker
container using `docker-compose up -d --build`

## Tests
```
RUSTFLAGS="${RUSTFLAGS:-} -D warnings" cargo test
```

## TODOs:
 - Find a way to mock HTTP Requests
 - Refactor to better support other CI tools and SCM
