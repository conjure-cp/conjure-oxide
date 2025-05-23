
# 1) build-environment: a container that sets up the build environment needed
#    to compile conjure oxide. 
#
#    This stage can be used as the container for Github workflows that involve
#    building Conjure Oxide e.g. nightly releases.
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

# install conjure from latest release
RUN mkdir -p conjure-build;
WORKDIR /conjure-build

RUN wget https://github.com/conjure-cp/conjure/releases/download/v2.5.1/conjure-v2.5.1-linux-with-solvers.zip &&\
    unzip conjure-v2.5.1-linux-with-solvers.zip &&\
    mv conjure-v2.5.1-linux-with-solvers/* . &&\
    rm -rf conjure-v2.5.1-linux-with-solvers*


###########################################################
# 3) a container that contains conjure oxide and conjure.

# as we are no longer building, we can use a more modern version of Linux :)
FROM ubuntu:latest

# java for savilerow
RUN apt-get update && apt-get install -y openjdk-21-jdk;

RUN mkdir -p /opt/conjure;
WORKDIR /opt/conjure

COPY --from=builder build/target/release/conjure_oxide .
COPY --from=builder conjure-build/ .

# see https://github.com/conjure-cp/conjure/blob/main/Dockerfile
ENV PATH /opt/conjure:$PATH
ENV LD_LIBRARY_PATH /opt/conjure/lib:$LD_LIBRARY_PATH
ENV MZN_STDLIB_DIR /opt/conjure/share/minizinc

RUN mkdir -p /root/;
WORKDIR /root/
