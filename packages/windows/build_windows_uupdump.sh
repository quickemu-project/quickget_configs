#!/bin/bash

set -e
cd /build/uupdump_converter

case "${ARCH}" in
    "x86_64") ARCH="amd64";;
    "aarch64") ARCH="arm64";;
    *) echo "Unsupported architecture: ${ARCH}"
       exit 1
       ;;
esac

RELEASE="${RELEASE/-build/}"

case "${RELEASE}" in
    "retail") RING="retail";;
    "release_preview") RING="rp";;
    "beta") RING="wis";;
    "dev") RING="wif";;
    "canary") RING="canary";;
    *) echo "Unsupported release: ${RELEASE}"
       exit 1
       ;;
esac

case "${EDITION}" in
    "Arabic") LANG="ar-sa";;
    "Brazilian Portuguese") LANG="pt-br";;
    "Bulgarian") LANG="bg-bg";;
    "Chinese (Simplified)") LANG="zh-cn";;
    "Chinese (Traditional)") LANG="zh-tw";;
    "Croatian") LANG="hr-hr";;
    "Czech") LANG="cs-cz";;
    "Danish") LANG="da-dk";;
    "Dutch") LANG="nl-nl";;
    "English International") LANG="en-gb";;
    "English (United States)") LANG="en-us";;
    "Estonian") LANG="et-ee";;
    "Finnish") LANG="fi-fi";;
    "French") LANG="fr-fr";;
    "French Canadian") LANG="fr-ca";;
    "German") LANG="de-de";;
    "Greek") LANG="el-gr";;
    "Hebrew") LANG="he-il";;
    "Hungarian") LANG="hu-hu";;
    "Italian") LANG="it-it";;
    "Japanese") LANG="ja-jp";;
    "Korean") LANG="ko-kr";;
    "Latvian") LANG="lv-lv";;
    "Lithuanian") LANG="lt-lt";;
    "Norwegian") LANG="nb-no";;
    "Polish") LANG="pl-pl";;
    "Portuguese") LANG="pt-pt";;
    "Romanian") LANG="ro-ro";;
    "Russian") LANG="ru-ru";;
    "Serbian Latin") LANG="sr-latn-rs";;
    "Slovak") LANG="sk-sk";;
    "Slovenian") LANG="sl-si";;
    "Spanish") LANG="es-es";;
    "Spanish (Mexico)") LANG="es-mx";;
    "Swedish") LANG="sv-se";;
    "Thai") LANG="th-th";;
    "Turkish") LANG="tr-tr";;
    "Ukrainian") LANG="uk-ua";;
    *) echo "Unsupported language: ${EDITION}"
       exit 1
       ;;
esac

# Fetchupd in UUPDump's JSON API doesn't appear to work correctly. Therefore, parse from website HTML instead.
UPD_ID="$(curl -s "https://uupdump.net/fetchupd.php?arch=${ARCH}&ring=${RING}" | grep -i -v "update" | grep -A 7 -e ", version" -e "Insider Preview" | grep -P -o '(?<=<code>)[0-9a-f-]{36}(?=</code)')"

if [ -z "${UPD_ID}" ]; then
    echo "Failed to fetch update ID."
    exit 1
fi

echo "Found update ID: ${UPD_ID}"
# Wait for ratelimiting
sleep 5

curl -s "https://uupdump.net/get.php?id=${UPD_ID}&pack=${LANG}&edition=core;professional&aria2=2" -o "aria2_script.txt"

echo "Downloading UUP files..."
aria2c --no-conf --console-log-level=warn --log-level=info -x16 -s16 -j5 -c -R -d"UUPs" -i"aria2_script.txt"

echo "Building image from UUP files..."
./convert.sh

EDITION="$(echo "${EDITION}" | sed 's/ /_/g' | sed 's/[()]//g')"

for ISO in *.ISO; do
    chmod a+rw "${ISO}"
    mv "${ISO}" "/output/windows-${RELEASE}-${EDITION}.iso"
    break;
done
