# Release Management

libmicrovmi project release is handled in the [CI](https://github.com/Wenzel/libmicrovmi/blob/master/.github/workflows/ci.yml), on Github Actions.

If a commit is pushed with a tag matching `v*`, the `release` job of the CI is executed,
as well as all jobs depending on it.

Release CI related jobs

- `release`: create a Github release
  - `release_debian`: add a Debian package to the Github release
  - `release_book`: build and publish the book
  - `publish`: publish the crate on crates.io
  - `publis_pypi`: publish the Python bindings on PyPI

## How to make a new release

Release and tags are managed using the [cargo-release](https://github.com/sunng87/cargo-release) tool.

~~~
$ cargo release --no-dev-version --workspace --skip-push  --execute
~~~

We skip-push the commit because there is a [bug](https://github.com/crate-ci/cargo-release/issues/222) in cargo-release when working
with a workspace.

amend the commit with the right tag
~~~
$ git commit --amend
edit with vxxxx
$ git push origin master
$ git push origin vxxxx
~~~

Note: `cargo-release` can handle the publication on crates.io, but we prefer to manage everything in one place, using the CI.
Therefore, publishing has been explicitely disabled in `Cargo.toml` for this tool, so no mistakes can happen.
