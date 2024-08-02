FROM debian:latest

COPY build_elementary.sh /build/
COPY os /build/os/

# Install dependencies and handle other preconditions
RUN apt-get update; apt-get install -y live-build patch gnupg2 binutils zstd

ENTRYPOINT ["/build/build_elementary.sh"]
