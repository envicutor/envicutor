#!/bin/bash

echo "Executing correctness tests"
node test.js
echo "Executing concurrency tests"
node concurrency.js
