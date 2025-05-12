#!/usr/bin/env bash

set -e

# From coverage testing
sudo virsh destroy coverage-test || true
sudo virsh undefine coverage-test || true

# From package testing
sudo virsh destroy arch-package-test || true
sudo virsh undefine arch-package-test || true
sudo virsh destroy debian-package-test || true
sudo virsh undefine debian-package-test || true
sudo virsh destroy ubuntu-package-test || true
sudo virsh undefine ubuntu-package-test || true
# sudo virsh destroy fedora-package-test || true
# sudo virsh undefine fedora-package-test || true
# sudo virsh destroy alma-package-test || true
# sudo virsh undefine alma-package-test || true
# sudo virsh destroy rocky-package-test || true
# sudo virsh undefine rocky-package-test || true

rm -rf images/*-test.qcow2
rm -f vendor.tar.gz
rm -f sandbox-*.crate