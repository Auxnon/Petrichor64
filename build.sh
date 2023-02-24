#!/bin/bash
version=$(node build-tools/version_reader.js)
a_silicon_zip="Petrichor64 (Apple Silicon) v${version}.dmg"
a_intel_zip="Petrichor64 (Apple Intel) v${version}.dmg"
home=$(pwd)

rm "$home"/dist/"$a_silicon_zip"
rm /tmp/tmp.dmg
cargo bundle --release
cd "$home"/dist
rm -rf Petrichor64.app
cp -r ../target/release/bundle/osx/Petrichor64.app .
codesign -s "Nick McAvoy" Petrichor64.app
appdmg ../build-tools/dmg-config.json "$a_silicon_zip"
codesign -s "Nick McAvoy" "$a_silicon_zip"
rm -rf Petrichor64.app
cd "$home"

#Apple Development: nicholasmcavoy89@gmail.com (M7KS95955P)
rm "$home"/dist/"$a_intel_zip"
rm /tmp/tmp.dmg
cargo bundle --profile release-nightly --target x86_64-apple-darwin
cd "$home"/dist
rm -rf Petrichor64.app
cp -r ../target/x86_64-apple-darwin/release-nightly/bundle/osx/Petrichor64.app .
codesign -s "Nick McAvoy" Petrichor64.app
appdmg ../build-tools/dmg-config.json "$a_intel_zip"
codesign -s "Nick McAvoy" "$a_intel_zip"
rm -rf Petrichor64.app
cd "$home"