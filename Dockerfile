
# 1) build-environment: a container that sets up the build environment needed
#    to compile conjure oxide.
#
#    This stage can be used as the container for Github workflows that involve
#    building Conjure Oxide e.g. nightly releases, testing.
#
#    This uses an older version of glibc, 2.8, to ensure the build binary is
#    widely runnable on many linux systems. For more details on supported
#    systems, see manylinux_2_28 documentation.

# --platform=$TARGETPLATFORM is for podman compatibility
FROM --platform=$TARGETPLATFORM 'quay.io/pypa/manylinux_2_28' as build-environment
ARG TARGETPLATFORM

# download wget for downloading node below, and zip for our nightly build CI.
RUN yum install -y wget zip;

# llvm / clang: for C++ dependencies (Minion, SAT) and bindgen.
# using clang not gcc as Rust's bindgen library requires libclang
RUN yum install -y llvm-toolset;

# nodejs: required to build treesitter grammar

# treesitter builds fail on the version of node found in this containers
# package manager, as it is very old. Installing node from a binary download
# instead.


# FIXME: Conjure has no linux/arm64 builds yet, so neither can we! When Conjure
# gets these, we can trivially make this container multi-platform by commenting
# out the below elif.

RUN if [ "$TARGETPLATFORM"  == "linux/amd64" ]; then ARCH="x64";\
    # elif [ "$TARGETPLATFORM" = "linux/arm64" ]; then ARCH="arm64";\
    else exit 1; fi;\
    wget https://nodejs.org/dist/v22.16.0/node-v22.16.0-linux-${ARCH}.tar.xz &&\
    tar -xf node-v22.16.0-linux-${ARCH}.tar.xz &&\
    cp node-v22.16.0-linux-${ARCH}/bin/* /usr/local/bin &&\
    cp -r node-v22.16.0-linux-${ARCH}/share/* /usr/local/share &&\
    cp -r node-v22.16.0-linux-${ARCH}/include/* /usr/local/include &&\
    cp -r node-v22.16.0-linux-${ARCH}/lib/* /usr/local/lib &&\
    rm -rf node-v22.16.0*;



# rustup
RUN curl https://sh.rustup.rs -sSf | sh -s -- -y;

ENV PATH "/root/.cargo/bin:$PATH"

###########################################################
# 2) builder: a container that builds conjure oxide.

FROM build-environment as builder

# grab conjure oxide source
WORKDIR /build
COPY . .
RUN git submodule update  --init --remote --recursive;

RUN cargo build --release;

###########################################################
# 3) a container that contains conjure oxide and conjure.

FROM ghcr.io/conjure-cp/conjure:main

# conjure should do this already, but for forwards compatibility
RUN mkdir -p /opt/conjure;
ENV PATH /opt/conjure:$PATH

COPY --from=builder /build/target/release/conjure-oxide /opt/conjure/conjure-oxide
