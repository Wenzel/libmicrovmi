# Python Bindings

## Nox

The project uses [Nox](https://nox.thea.codes/en/stable/) to facilite and automate the developer workflow.
Please install this tool before you start

Running `nox` without any argument will run the default sessions.

## Generating the Wheels

Distributing a Python native extension compatible with many systems and a large set of Python interpreters is a challenging task.

The [manylinux](https://github.com/pypa/manylinux) project comes to the rescue here.

The extension is built based on the [`manylinux2014`](https://www.python.org/dev/peps/pep-0599/) platform tag.

Generation of the wheels is managed by `nox` and requires [`Docker`](https://www.docker.com/) to build a custom manylinux2014 CentOS image, and
execute the script inside it.

To start the generation:

~~~
$ cd libmicrovmi/python
$ nox -r -s generate_wheels -- --features xen
~~~

you can activate more drivers

~~~
$ nox -r -s generate_wheels -- --features xen,kvm,virtualbox
~~~

and enable the release mode as well

~~~
nox -r -s generate_wheels -- --features xen --release
~~~

After the execution, the wheels will be available in `libmicrovmi/python/dist/manylinux`.

## Testing Volatility

Nox provides sessions to facilitate testing the volatility integration on a given driver.

For Xen:
~~~
nox -r -s test_volatility_xen -- vmi:///....
~~~

To list nox sessions:
~~~
nox -l
~~~
