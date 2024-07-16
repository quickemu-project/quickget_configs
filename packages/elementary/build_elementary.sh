#!/bin/bash

cd /build/os || exit

if [ ! -d /output ]; then
    echo "A directory must be mounted at /output. If quickget is building this image, please file an issue."
    exit 1
fi

case "${RELEASE}" in
    "8-daily") ./build.sh etc/terraform-daily-8.0-azure.conf;;
    "stable") ./build.sh;;
esac

for file in builds/*/*.iso; do
    mv "${file}" "/output/elementary-${RELEASE}.iso"
    chmod a+rw "/output/elementary-${RELEASE}.iso"
done
