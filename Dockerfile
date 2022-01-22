FROM konstin2/maturin:latest
ARG FFMPEG_VERSION="4.4"

# build tools
RUN echo "[1] Install dependencies"
RUN yum install -y autoconf automake bzip2 bzip2-devel clang freetype-devel gcc gcc-c++ git libtool make mercurial pkgconfig zlib-devel

WORKDIR ~/ffmpeg_sources

# nasm
RUN echo "[2] Install nasm" && \
    curl -O -L https://www.nasm.us/pub/nasm/releasebuilds/2.14.02/nasm-2.14.02.tar.gz && \
    tar xvfz nasm-2.14.02.tar.gz && \
    cd nasm-2.14.02 && \
    ./configure --prefix="$HOME/ffmpeg_build" --bindir="$HOME/bin" && \
    make -j$(nproc) && \
    make install

# yasm
RUN echo "[3] Install yasm" && \
    curl -O -L https://www.tortall.net/projects/yasm/releases/yasm-1.3.0.tar.gz  --insecure && \
    tar xzf yasm-1.3.0.tar.gz && \
    cd yasm-1.3.0 && \
    ./configure --prefix="$HOME/ffmpeg_build" --bindir="$HOME/bin" && \
    make -j$(nproc) && \
    make install

# libx264
RUN echo "[4] Install libx264" && \
    git clone --depth 1 https://code.videolan.org/videolan/x264.git && \
    cd x264 && \
    export PATH="$HOME/bin:$PATH" && \
    PKG_CONFIG_PATH="$HOME/ffmpeg_build/lib/pkgconfig" ./configure --prefix="$HOME/ffmpeg_build" --bindir="$HOME/bin" --enable-shared --enable-pic && \
    make -j$(nproc) && \
    make install

# libvpx
RUN echo "[5] Install libvpx" && \
    git clone --depth 1 https://chromium.googlesource.com/webm/libvpx.git && \
    cd libvpx && \
    export PATH="$HOME/bin:$PATH" && \
    ./configure --prefix="$HOME/ffmpeg_build" --disable-examples --disable-unit-tests --enable-vp9-highbitdepth --as=yasm --enable-shared --enable-pic && \
    make -j$(nproc) && \
    make install

# ffmpeg
RUN echo "[6] Install ffmpeg" && \
    curl -O -L https://ffmpeg.org/releases/ffmpeg-$FFMPEG_VERSION.tar.bz2 && \
    tar xjf ffmpeg-$FFMPEG_VERSION.tar.bz2 && \
    cd ffmpeg-$FFMPEG_VERSION && \
    export PATH="$HOME/bin:$PATH" && \
    PKG_CONFIG_PATH="$HOME/ffmpeg_build/lib/pkgconfig" ./configure \
      --prefix="$HOME/ffmpeg_build" \
      --extra-cflags="-I$HOME/ffmpeg_build/include" \
      --extra-ldflags="-L$HOME/ffmpeg_build/lib" \
      --extra-libs=-lpthread \
      --extra-libs=-lm \
      --bindir="$HOME/bin" \
      --enable-gpl \
      --enable-libvpx \
      --enable-libx264 \
      --enable-nonfree \
      --disable-static \
      --enable-shared \
      --enable-pic && \
    make -j$(nproc) && \
    make install

ENV FFMPEG_DIR "/root/ffmpeg_build"
ENV LD_LIBRARY_PATH /root/ffmpeg_build/lib:$LD_LIBRARY_PATH

WORKDIR /io
