#!/bin/bash
version=$(node version_reader.js)
a_silicon_zip="Petrichor64 (Apple Silicon) v${version}.zip"
a_intel_zip="Petrichor64 (Apple Intel) v${version}.zip"

cargo bundle --release
cd target/release/bundle/osx
codesign -s "Apple Development: nicholasmcavoy89@gmail.com (M7KS95955P)" Petrichor64.app
rm ../../../../dist/"$a_silicon_zip"
zip -r ../../../../dist/"$a_silicon_zip" Petrichor64.app
cd ../../../../


cargo bundle --release --target x86_64-apple-darwin
cd target/x86_64-apple-darwin/release/bundle/osx
codesign -s "Apple Development: nicholasmcavoy89@gmail.com (M7KS95955P)" Petrichor64.app
rm ../../../../../dist/"$a_intel_zip"
zip -r ../../../../../dist/"$a_intel_zip" Petrichor64.app
cd ../../../../../