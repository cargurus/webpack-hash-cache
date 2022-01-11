#!/bin/bash

COMMIT_MESSAGE=$(git log --format=%B --no-merges -n 1 | tr -d '\n')
if [[ ${COMMIT_MESSAGE} =~ "[publish binary]" ]]
    then
        npm
        echo -e "\n_authToken=${secrets.NPM_TOKEN}" >> .npmrc
        npm upload-binary
        npm clean
	      npm publish
        git checkout .npmrc
    else
        echo "skipping publish";
        exit 0;
    fi
