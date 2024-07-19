#!/bin/bash

set -xe

if [ ! -d FastLZ ]; then
    git clone https://github.com/ariya/FastLZ.git
fi

cc -IFastLZ -Wall -O2 -o testtool tool.c FastLZ/fastlz.c
