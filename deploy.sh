#!/bin/bash

COMMIT_MESSAGE=$(git log --format=%B --no-merges -n 1 | tr -d '\n')
if [[ ${COMMIT_MESSAGE} =~ "[publish binary]" ]]
    then
        yarn
        echo -e "\n_authToken=${NPM_TOKEN}" >> .npmrc
        yarn upload-binary
        yarn publish
        git checkout .npmrc
    else
        echo "skipping publish";
        exit 0;
    fi