#!/bin/bash

echo "------ BUILDING ------"
cd vendor || exit
cmake -B build -S .
cmake --build build
