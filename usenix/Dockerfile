# Copyright 2020 Google LLC
#
# Licensed under the Apache License, Version 2.0 (the "License");
# you may not use this file except in compliance with the License.
# You may obtain a copy of the License at
# # http://www.apache.org/licenses/LICENSE-2.0
#
# Unless required by applicable law or agreed to in writing, software
# distributed under the License is distributed on an "AS IS" BASIS,
# WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
# See the License for the specific language governing permissions and
# limitations under the License.

FROM ubuntu:bionic


RUN apt-get update -y &&  \
    apt-get -y install wget python-pip python3-setuptools apt-transport-https \
    llvm-6.0 llvm-6.0-dev clang-6.0 llvm-6.0-tools libboost-all-dev texinfo \
    lsb-release zip llvm-dev cmake software-properties-common autoconf curl zlib1g-dev flex bison git ragel

RUN DEBIAN_FRONTEND="noninteractive" apt-get update && DEBIAN_FRONTEND="noninteractive" apt-get -yq --no-install-recommends install tcl

#install cargo
#RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | bash -s -- -y

#install protobuf
#RUN wget https://apt.llvm.org/llvm.sh && ct clone https://github.com/xiph/vorbis.githmod +x llvm.sh && ./llvm.sh 9
#RUN wget https://apt.llvm.org/llvm.sh && chmod +x llvm.sh && ./llvm.sh 12
#RUN ln -s /usr/bin/llvm-config-12 /usr/bin/llvm-config
#RUN ln -s /usr/bin/clang-12 /usr/bin/clang
#RUN ln -s /usr/bin/clang++-12 /usr/bin/clang++


RUN git clone https://github.com/protocolbuffers/protobuf.git /protobuf  && \
    cd /protobuf && \
    git submodule update --init --recursive && \
    unset CFLAGS && \
    unset CXXFLAGS && \
    ./autogen.sh && \
    ./configure  && \
   # ./configure  && \
    make -j && \
    make install && \
    ldconfig

RUN git clone https://github.com/Z3Prover/z3.git /z3 && \
		cd /z3 && git checkout z3-4.8.7 && mkdir -p build && cd build && \
		cmake .. && make -j && make install
		#cmake .. && make -j && make install
RUN ldconfig

#install tcmallo
RUN git clone https://github.com/gperftools/gperftools.git /gperftools && \
    cd /gperftools && \
    unset CFLAGS && \
    unset CXXFLAGS && \
    ./autogen.sh && \
    ./configure  && \
    #./configure && \
    make -j && \
    make install && \ 
    ldconfig

#install AFL++
#RUN git clone  https://github.com/AFLplusplus/AFLplusplus.git /afl && \
#    cd /afl && \
#    git checkout f41aafa4f7aa446c3cb1cbe6d77364cf32a6c6cb && \
#    unset CFLAGS && \
#    unset CXXFLAGS && \
#    AFL_NO_X86=1 PYTHON_INCLUDE=/ make -j && make install && \
#    make -C examples/aflpp_driver && \
#    cp examples/aflpp_driver/libAFLDriver.a /


RUN rm -rf /usr/local/include/llvm && rm -rf /usr/local/include/llvm-c
RUN rm -rf /usr/include/llvm && rm -rf /usr/include/llvm-c
RUN ln -s /usr/lib/llvm-6.0/include/llvm /usr/include/llvm
RUN ln -s /usr/lib/llvm-6.0/include/llvm-c /usr/include/llvm-c
# build kirenenko
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | bash -s -- --default-toolchain=1.61.0 -y
ENV PATH="/root/.cargo/bin:${PATH}"
RUN apt-get -y install libc++-dev libc++abi-dev
RUN git clone https://github.com/r-fuzz/fastgen.git /symsan
RUN cd /symsan && \
    unset CFLAGS && \
    unset CXXFLAGS && \
    ./build/build.sh


RUN git clone https://github.com/chenju2k6/build-programs.git /src/build-programs
RUN cd /src/build-programs && ./build_symsan.sh

RUN apt-get install -y cargo clang-10 libpixman-1-dev cmake g++ git llvm-10-dev llvm-10-tools ninja-build python3-pip zlib1g-dev
RUN pip3 install lit
RUN pip3 install jinja2
RUN pip install jinja2
RUN git clone https://github.com/eurecom-s3/symcc.git /symcc
RUN cd /symcc && git checkout 07c8895fea8e5fae90417df60a130be7a9c63d92  && git submodule update --init && \
    mkdir build && cd build && cmake -G Ninja -DQSYM_BACKEND=ON ../ && ninja all &&  \
    cd .. && mkdir build_simple && cd build_simple && cmake -G Ninja  ../ && ninja all 

RUN cd /src && wget https://github.com/Kitware/CMake/releases/download/v3.24.0-rc1/cmake-3.24.0-rc1-linux-x86_64.tar.gz && tar xvf cmake-3.24.0-rc1-linux-x86_64.tar.gz
RUN git clone --depth 1 https://github.com/llvm/llvm-project.git /llvm-project
RUN mkdir -p /src/libcxx_symcc && cd /src/libcxx_symcc/ && \ 
    SYMCC_REGULAR_LIBCXX=1 SYMCC_NO_SYMBOLIC_INPUT=1  /src/cmake-3.24.0-rc1-linux-x86_64/bin/cmake -G Ninja /llvm-project/llvm \
    -DLLVM_ENABLE_PROJECTS="libcxx;libcxxabi" \
    -DLLVM_TARGETS_TO_BUILD="X86" \
    -DLLVM_DISTRIBUTION_COMPONENTS="cxx;cxxabi;cxx-headers" \
    -DCMAKE_BUILD_TYPE=Release \
    -DCMAKE_INSTALL_PREFIX=/src/libcxx_symcc/install \
    -DCMAKE_C_COMPILER=/symcc/build_simple/symcc \
    -DCMAKE_CXX_COMPILER=/symcc/build_simple/sym++ && SYMCC_REGULAR_LIBCXX=1 SYMCC_NO_SYMBOLIC_INPUT=1 ninja distribution && SYMCC_REGULAR_LIBCXX=1 SYMCC_NO_SYMBOLIC_INPUT=1 ninja install-distribution


RUN git clone https://github.com/eurecom-s3/symqemu.git /symqemu
RUN cd /symqemu && git checkout d18384493222cb830004852e0968084da3aa1db8 && \
    mkdir build && cd build && ../configure                                                    \
      --audio-drv-list=                                           \
      --disable-bluez                                             \
      --disable-sdl                                               \
      --disable-gtk                                               \
      --disable-vte                                               \
      --disable-opengl                                            \
      --disable-virglrenderer                                     \
      --disable-werror                                            \
      --target-list=x86_64-linux-user                             \
      --enable-capstone=git                                       \
      --symcc-source=/symcc                  \
      --symcc-build=/symcc/build && make -j

WORKDIR /out
RUN cd /out && wget https://jigsaw.cs.ucr.edu/seeds.tar.gz && tar xvf seeds.tar.gz


COPY symsan_nbench_makefile /src
COPY symcc_nbench_makefile /src
RUN cd /src && wget http://www.math.utah.edu/~mayer/linux/nbench-byte-2.2.3.tar.gz && tar xvf nbench-byte-2.2.3.tar.gz && cp -r nbench-byte-2.2.3 nbench_native && \
    cp -r nbench_native nbench_symsan && cp -r nbench_native nbench_symcc && \
    cd /src/nbench_native && make && \
    cd /src/nbench_symsan && cp /src/symsan_nbench_makefile Makefile && KO_DONT_OPTMIZE=1 KO_CC=clang-6.0 make && \
    cd /src/nbench_symcc && cp /src/symcc_nbench_makefile Makefile && SYMCC_NO_SYMBOLIC_INPUT=1 make 

RUN pip install xlsxwriter pycrypto

RUN git clone https://github.com/chenju2k6/build-programs.git /src/build-programs-symcc
RUN cd /src/build-programs-symcc && ./build_symcc.sh

RUN git clone https://github.com/chenju2k6/build-programs.git /src/build-programs-native
RUN cd /src/build-programs-native && ./build_native.sh

#RUN cd /src && git clone https://github.com/chenju2k6/cgc_programs cgc_programs5 && cd cgc_programs5 && ./build.sh

COPY kirenenko.patch /src
RUN git clone https://github.com/ChengyuSong/Kirenenko.git /kirenenko && cd /kirenenko && patch -p1 < /src/kirenenko.patch  && unset CFLAGS && unset CXXFLAGS && ./build/build.sh
RUN rm -rf /usr/lib/x86_64-linux-gnu/libz3*
RUN cd /src && git clone https://github.com/chenju2k6/cgc_programs && cd cgc_programs && ./build_symcc.sh  && ./build.sh && ./build_symsan.sh && ./build_symsannosolve.sh
#RUN cd /src && git clone https://github.com/chenju2k6/cgc_programs cgc_programs6 && cd cgc_programs6 && ./build_symsan.sh

COPY nbench.patch /src
RUN cd /symsan && patch -p1 < /src/nbench.patch && unset CFLAGS && unset CXXFLAGS && ./build/build.sh
RUN git clone https://github.com/chenju2k6/build-programs.git /src/build-programs-symsannosolve
RUN cd /src/build-programs-symsannosolve && ./build_symsan.sh
COPY run_native.sh /src/cgc_programs/build/
COPY run_symsan.sh /src/cgc_programs/build_symsannosolve/
COPY run_symsan.sh /src/cgc_programs/build_symsan/
COPY run_symcc.sh /src/cgc_programs/build_symcc/
COPY run_symqemu.sh /src/cgc_programs/build/
COPY cgc_list /src/
COPY run_symsan_time.sh /src/build-programs-symsannosolve/
COPY run_symcc_time.sh /src/build-programs-symcc/
COPY run_symqemu_time.sh /src/build-programs-native/
COPY run_native_time.sh /src/build-programs-native/
#mem
COPY run_symcc_mem.sh /src/build-programs-symcc/
COPY run_native_mem.sh /src/build-programs-native/
COPY run_symsan_mem.sh /src/build-programs-symsannosolve/
RUN apt-get install -y time
COPY symcc_nosolve.patch /src
COPY symsan_cov.patch /src
COPY fuzzer.sh /src/build-programs/
