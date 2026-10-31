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

RUN rustup default stable \
    && cargo install cbindgen \
    && rustup component add clippy rustfmt
