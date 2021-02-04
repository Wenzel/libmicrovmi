FROM quay.io/pypa/manylinux2014_x86_64

# install clang
RUN yum -y install clang

# libkvmi v6
RUN git clone -b kvmi-v6 https://github.com/bitdefender/libkvmi.git \
    && cd libkvmi \
    && ./bootstrap \
    && ./configure \
    && make \
    && make install \
    && cd .. \
    && rm -rf libkvmi

# libFDP.so
RUN git clone --depth 1 https://github.com/thalium/icebox \
    && cd icebox/src/FDP \
    && g++ -std=c++11 -shared -fPIC FDP.cpp -o libFDP.so \
    && mv include/* /usr/local/include/ \
    && mv libFDP.so /usr/local/lib/ \
    && cd - \
    && rm -rf icebox

