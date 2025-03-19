FROM ubuntu:22.04

RUN \
    && apt update
    && apt install -y build-essential python3-dev python3-setuptools make cmake ffmpeg libavcodec-dev libavfilter-dev libavformat-dev libavutil-dev libswresample-dev
