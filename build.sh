#!/bin/bash
version=$(node build-tools/version_reader.js)
a_silicon_zip="Petrichor64 (Apple Silicon) v${version}.dmg"
a_intel_zip="Petrichor64 (Apple Intel) v${version}.dmg"
a_headless_zip="Petrichor64 (Apple Headless) v${version}.zip"
home=$(pwd)
# signature="Apple Development: nicholasmcavoy89@gmail.com (M7KS95955P)"
#Nick McAvoy
signature="Nick McAvoy"

rm "$home"/dist/"$a_silicon_zip"
rm /tmp/tmp.dmg
cargo bundle --release
cd "$home"/dist
cp -r ../target/release/bundle/osx/Petrichor64.app .
codesign -s "$signature" Petrichor64.app
appdmg ../build-tools/dmg-config.json "$a_silicon_zip"
# codesign -s "$signature" "$a_silicon_zip"
rm -rf Petrichor64.app
cd "$home"

rm "$home"/dist/"$a_intel_zip"
rm /tmp/tmp.dmg
cargo bundle --profile release-nightly --target x86_64-apple-darwin
cd "$home"/dist
cp -r ../target/x86_64-apple-darwin/release-nightly/bundle/osx/Petrichor64.app .
codesign -s "$signature" Petrichor64.app
appdmg ../build-tools/dmg-config.json "$a_intel_zip"
# codesign -s "$signature" "$a_intel_zip"
rm -rf Petrichor64.app
cd "$home"

rm "$home"/dist/"$a_headless_zip"
cargo build --release --no-default-features --features online_capable
cd "$home"/dist
cp -r ../target/release/Petrichor64 petrichor64_headless
codesign -s "$signature" petrichor64_headless
zip "$a_headless_zip" petrichor64_headless
rm -rf petrichor64_headless
cd "$home"