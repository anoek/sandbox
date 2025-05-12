#!/usr/bin/env bash

set -e
. common-functions.sh

common_init


reinitialize_fs mkfs.ext2
do_test /home/sandbox/source

reinitialize_fs mkfs.ext3
do_test /home/sandbox/source

reinitialize_fs mkfs.ext4
do_test /home/sandbox/source

reinitialize_as_nfs
do_test /home/sandbox/source

reinitialize_fs mkfs.btrfs
do_test /home/sandbox/source

reinitialize_fs mkfs.xfs
do_test /home/sandbox/source



