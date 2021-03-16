# Python Bindings

To generate the wheels

~~~shell
docker build -t manylinux2014_microvmi .
cd ..   # go back to project root
docker run --rm -v `pwd`:/io manylinux2014_microvmi /io/python/build-wheels.sh --features xen,kvm,virtualbox --release
~~~
