#!/usr/bin/env bash

set -e
. common-functions.sh

common_init


reinitialize_fs mkfs.btrfs
do_test /home/sandbox/source/

## Generate the coverage report
ssh -o "StrictHostKeyChecking no" -i generated/id_sandbox_test_vm_key sandbox@$COVERAGE_IP 'bash -s' <<EOF
    set -e
    . .cargo/env
    cd /home/sandbox/source/
    mkdir -p coverage/profdata
    cp /home/sandbox/merged-*.profdata /home/sandbox/source/coverage/profdata/
    rm -Rf /home/sandbox/coverage/profraw
    make update-coverage-report
EOF

scp -o "StrictHostKeyChecking no" -i generated/id_sandbox_test_vm_key -rp sandbox@$COVERAGE_IP:/home/sandbox/source/coverage/html coverage_html

