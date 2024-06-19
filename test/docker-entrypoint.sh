#!/bin/bash

echo "Executing correctness tests"
node test.js || exit 1
echo "Executing concurrency tests"
node concurrency.js || exit 1
