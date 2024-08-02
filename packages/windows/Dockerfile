FROM ubuntu:latest

RUN apt-get update; apt-get install -y aria2 cabextract wimtools chntpw genisoimage curl

COPY uupdump_converter/ /build/uupdump_converter/
COPY build_windows_uupdump.sh /build/

ENTRYPOINT ["/build/build_windows_uupdump.sh"]
