FROM quay.io/pypa/manylinux2014_x86_64:latest@sha256:3eee053a02bd2372799fe1fdbc256245996f66dff5d4d47173429ae7d09dd9bf

# install libclang 7.0
RUN yum -y install llvm-toolset-7.0-clang llvm-toolset-7.0-clang-devel

# Xen
# Note: the first sed disables a compiler flag that would be treated as an error
# the second sed disabled the compilation of qemu, which is very long and also
# had errors, and we don't need it anyway
RUN git clone -b RELEASE-4.17.0 --depth 1 https://github.com/xen-project/xen \
        && cd xen \
        && yum install -y flex ninja-build dev86 xz-devel python3-devel gettext-devel iasl \
        ncurses-devel pixman-devel wget yajl-devel zlib-devel \
        glibc-devel.i686 libuuid-devel \
        && ./configure --disable-xen --disable-docs --disable-stubdom \
        --enable-tools --disable-rombios \
        && make -j4 dist-tools \
        && make install-tools \
        && cd .. \
        && rm -rf xen

# libkvmi v7
RUN git clone https://github.com/bitdefender/libkvmi.git \
        && cd libkvmi \
        && git checkout bf5776319e1801b59125c994c459446f0ed6837e \
        && ./bootstrap \
        && ./configure \
        && make \
        && make install \
        && cd .. \
        && rm -rf libkvmi

# libFDP.so
RUN git clone --depth 1 https://github.com/thalium/icebox \
        && cd icebox/src/FDP \
        && g++ -std=c++11 -shared -fPIC FDP.cpp -o libFDP.so -lrt \
        && mv include/* /usr/local/include/ \
        && mv libFDP.so /usr/local/lib/ \
        && cd - \
        && rm -rf icebox

