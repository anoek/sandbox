#!/usr/bin/env bash

set -e
. common-functions.sh

common_init

reinitialize_fs mkfs.ext4
do_test /home/sandbox/source/utf8/こんにちは

reinitialize_fs mkfs.ext4
do_test /home/sandbox/source/utf8/😀

reinitialize_fs mkfs.ext4
do_test /home/sandbox/source/one_dir_deep

# We test /home/sandbox/source in the filesystem test a lot, no need to test it here


### At this point coverage tests should be at 100% so lets bail early if they aren't
# ssh -o "StrictHostKeyChecking no" -i generated/id_sandbox_test_vm_key sandbox@$COVERAGE_IP 'bash -s' <<EOF
#     set -e
#     . .cargo/env
#     cd /home/sandbox/source/one_dir_deep
#     mkdir -p coverage/profdata
#     cp /home/sandbox/merged-*.profdata /home/sandbox/source/coverage/profdata/
#     make coverage-check
# EOF

