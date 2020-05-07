#!/bin/bash

COMMIT_MESSAGE=$(git log --format=%B --no-merges -n 1 | tr -d '\n')
if [[ ${COMMIT_MESSAGE} =~ "[publish binary]" ]]
    then
        yarn
        yarn upload-binary
        yarn publish
    else
        echo "skipping publish";
        exit 0;
    fi