#!/usr/bin/env bash

set -xe

if [ ! -f ../dist/*.tar.zst ]; then
    make -C ../ package
fi

MEM=4096
VCPUS=4
PACMAN_ARCHIVE=$(ls ../dist/*x86_64*.tar.zst)
DEB_ARCHIVE=$(ls ../dist/*x86_64*.deb)
RPM_ARCHIVE=$(ls ../dist/*x86_64*.rpm)
DEB_ARCHIVE_ARM64=$(ls ../dist/*arm64*.deb)

ENABLE_ARCH64=false


sudo virsh destroy arch-package-test || true
sudo virsh undefine arch-package-test || true
sudo virsh destroy debian-package-test || true
sudo virsh undefine debian-package-test || true
sudo virsh destroy ubuntu-package-test || true
sudo virsh undefine ubuntu-package-test || true
sudo virsh destroy fedora-package-test || true
sudo virsh undefine fedora-package-test || true

if $ENABLE_ARCH64; then
    sudo virsh destroy ubuntu-arm64-package-test || true
    sudo virsh undefine ubuntu-arm64-package-test || true

    sudo cp images/ubuntu24lts-arm64-base.img images/ubuntu-arm64-prepared.qcow2

    sudo virt-customize -a images/ubuntu-arm64-prepared.qcow2 --copy-in $DEB_ARCHIVE_ARM64:/  \
                    --firstboot-command "systemctl enable --now qemu-guest-agent" 

    sudo virt-install --name ubuntu-arm64-package-test \
                        --arch aarch64 \
                        --memory $MEM \
                        --vcpus $VCPUS \
                        --disk images/ubuntu-arm64-prepared.qcow2 \
                        --import \
                        --os-variant ubuntu24.04 \
                        --graphics none \
                        --noautoconsole
fi



sudo cp images/arch-base.qcow2 images/arch-prepared.qcow2
sudo cp images/debian-base.qcow2 images/debian-prepared.qcow2
sudo cp images/ubuntu24lts-base.img images/ubuntu-prepared.qcow2
# sudo cp images/fedora-base.qcow2 images/fedora-prepared.qcow2


sudo virt-customize -a images/arch-prepared.qcow2 --copy-in $PACMAN_ARCHIVE:/  \
                --run-command "systemctl enable qemu-guest-agent"  \
                --run-command "passwd -d root"

sudo virt-customize -a images/debian-prepared.qcow2 --copy-in $DEB_ARCHIVE:/  \
                --run-command "apt-get update" \
                --run-command "apt-get install -y qemu-guest-agent" \
                --run-command "systemctl enable qemu-guest-agent"  \
                --run-command "passwd -d root"

sudo virt-customize -a images/ubuntu-prepared.qcow2 --copy-in $DEB_ARCHIVE:/  \
                --run-command "apt-get update" \
                --run-command "apt-get install -y qemu-guest-agent" \
                --run-command "systemctl enable qemu-guest-agent"  \
                --run-command "passwd -d root"

# sudo virt-customize -a images/fedora-prepared.qcow2 --copy-in $RPM_ARCHIVE:/  \
#                 --run-command "systemctl enable qemu-guest-agent"  \
#                 --run-command "echo 'QEMU_GA_ARGS=--allow-rpcs=guest-exec,guest-exec-status' >>  /etc/sysconfig/qemu-ga" 





sudo virt-install --name arch-package-test \
                    --memory $MEM \
                    --vcpus $VCPUS \
                    --disk images/arch-prepared.qcow2 \
                    --import \
                    --os-variant archlinux \
                    --graphics none \
                    --noautoconsole

sudo virt-install --name debian-package-test \
                    --memory $MEM \
                    --vcpus $VCPUS \
                    --disk images/debian-prepared.qcow2 \
                    --import \
                    --os-variant debian12 \
                    --graphics none \
                    --noautoconsole

sudo virt-install --name ubuntu-package-test \
                    --memory $MEM \
                    --vcpus $VCPUS \
                    --disk images/ubuntu-prepared.qcow2 \
                    --import \
                    --os-variant ubuntu24.04 \
                    --graphics none \
                    --noautoconsole

# sudo virt-install --name fedora-package-test \
#                     --memory $MEM \
#                     --vcpus $VCPUS \
#                     --disk images/fedora-prepared.qcow2 \
#                     --import \
#                     --os-variant fedora41\
#                     --graphics none \
#                     --noautoconsole
