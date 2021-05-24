FROM gitpod/workspace-full

RUN sudo apt-get update \
    && sudo apt-get install -y --no-install-recommends clang libxen-dev \
    && sudo apt-get clean && sudo rm -rf /var/lib/apt/lists/

RUN git clone https://github.com/bitdefender/libkvmi.git \
    && cd libkvmi \
    && git checkout bf5776319e1801b59125c994c459446f0ed6837e \
    && ./bootstrap \
    && ./configure \
    && make \
    && sudo make install \
    && cd .. \
    && rm -rf libkvmi

RUN git clone --depth 1 https://github.com/thalium/icebox \
    && cd icebox/src/FDP \
    && g++ -std=c++11 -shared -fPIC FDP.cpp -o libFDP.so \
    && sudo mv include/* /usr/local/include/ \
    && sudo mv libFDP.so /usr/local/lib/ \
    && cd - \
    && rm -rf icebox

RUN rustup default stable \
    && cargo install cbindgen \
    && rustup component add clippy rustfmt
