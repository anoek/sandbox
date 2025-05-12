#!/usr/bin/env bash

set -e

MEM=24576
VCPUS=$(nproc)

sudo virsh destroy coverage-test || true
sudo virsh undefine coverage-test || true

rm -f sandbox-*.crate

if [ ! -f vendor.tar.gz ]; then
    cd ..
    cargo vendor
    tar -czvf vm-testing/vendor.tar.gz vendor
    cd vm-testing
fi

cd ..
cargo package --allow-dirty --no-verify
cp target/package/sandbox-*.crate vm-testing/
cd vm-testing

CRATE=$(ls sandbox-*.crate)

sudo cp images/coverage-prepared.qcow2 images/coverage-test.qcow2

cat <<EOF > generated/cargo_config.toml
[source.crates-io]
replace-with = "vendored-sources"

[source.vendored-sources]
directory = "vendor"
EOF



sudo virt-customize -a images/coverage-test.qcow2 \
    --copy-in vendor.tar.gz:/home/sandbox/ \
    --copy-in $CRATE:/home/sandbox/ \
    --copy-in generated/cargo_config.toml:/home/sandbox/

sudo virt-install --name coverage-test \
                    --memory $MEM \
                    --vcpus $VCPUS \
                    --disk images/coverage-test.qcow2 \
                    --import \
                    --os-variant ubuntu24.04 \
                    --network network=default \
                    --graphics none \
                    --noautoconsole

