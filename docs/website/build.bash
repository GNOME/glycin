#!/bin/bash -e

rm -rf public
mkdir public

cp docs/website/header.html public/index.html
./docs/website/list-formats.rs >> public/index.html
cat docs/website/footer.html >> public/index.html

cp docs/website/style.css public/
cp docs/website/icon.svg public/
